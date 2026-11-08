//! DOWNBEAT core: the engine-independent game model.
//!
//! ```text
//!  stream position ──> Conductor ──> chart time ──> Judge ──> Verdicts ──> Session
//!        (engine)       (clock)                    (windows)              (score/stats)
//!                          │
//!                          └──> ClockEvents (tick / beat / bar) ──> HUD, VFX, music stems
//! ```
//!
//! `Game` is the one object the engine talks to. The Godot glue (feature
//! `gdext`) wraps it as `DbGame`. The simulator (`sim`) drives it with a bot so
//! every mechanism is exercised headless.

pub mod chart;
pub mod clock;
pub mod judge;
pub mod analysis;
pub mod integrity;
pub mod mods;
pub mod patterns;
pub mod rating;
pub mod replay;
pub mod score;
pub mod sim;
/// The produced placeholder track (stems rendered by the core).
pub mod song;
/// C ABI for the web editor (wasm32); also compiles natively for its tests.
pub mod wasm;

#[cfg(feature = "gdext")]
pub mod godot;

use chart::{Chart, Level, Target};
use clock::{ClockConfig, ClockEvent, Conductor};
use judge::{Hit, Judge, Shot, Verdict, Windows};
use mods::Mods;
use replay::{Input, Replay};
use score::{Report, Scoring, Session};

#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    Clock(ClockEvent),
    Judge(Verdict),
    /// A set piece's bar arrived (index into `Chart::cues`).
    Cue(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BossStatus {
    pub phase: u32,
    pub phases: u32,
    pub name: String,
    pub kills: u32,
    pub total: u32,
    pub all_kills: u32,
    pub all_total: u32,
    pub phase_end_time: f64,
    pub dead: bool,
}

#[derive(Clone, Debug)]
pub struct Game {
    conductor: Conductor,
    judge: Judge,
    session: Session,
    scoring: Scoring,
    level: Option<Level>,
    inputs: Vec<Input>,
    events: Vec<GameEvent>,
    clock_buf: Vec<ClockEvent>,
    verdict_buf: Vec<Verdict>,
    /// Session as it stood when each encounter segment's first enemy spawned.
    checkpoints: Vec<(u32, Session)>,
    current_segment: Option<u32>,
    restarts: u32,
    mods: Mods,
    stars: f64,
    /// Downbeat volley: on-beat kills charge it; full at `VOLLEY_FULL`.
    volley_charge: u32,
    marked: Vec<u32>,
    /// Next cue to fire (cues are sorted by time; a restart rewinds this).
    next_cue: usize,
}

/// On-beat kills to charge the Downbeat volley.
pub const VOLLEY_FULL: u32 = 16;

impl Game {
    pub fn from_level(level: &Level, cfg: ClockConfig, windows: Windows, scoring: Scoring) -> Result<Self, chart::CompileError> {
        Self::from_level_with_mods(level, Mods::NONE, cfg, windows, scoring)
    }

    /// Build with mods: DT/HT change the tempo the chart is compiled at, HR
    /// shrinks weak points, NF sets `no_fail`. The fingerprint reflects the
    /// modded chart, so modded replays only re-score against the same mods.
    pub fn from_level_with_mods(level: &Level, mods: Mods, mut cfg: ClockConfig, windows: Windows, mut scoring: Scoring) -> Result<Self, chart::CompileError> {
        let mut modded = level.clone();
        modded.bpm *= mods.bpm_factor();
        let mut chart = chart::compile(&modded, &patterns::builtin_lib())?;
        if mods.hr {
            for t in &mut chart.targets {
                t.radius *= mods.radius_factor();
            }
        }
        scoring.no_fail = scoring.no_fail || mods.nf;
        cfg.bpm = modded.bpm;
        let mut g = Self::from_chart(chart, cfg, windows, scoring);
        g.level = Some(level.clone());
        g.mods = mods;
        Ok(g)
    }

    pub fn from_chart(chart: Chart, mut cfg: ClockConfig, windows: Windows, scoring: Scoring) -> Self {
        cfg.bpm = chart.bpm;
        Self {
            conductor: Conductor::new(cfg),
            judge: Judge::new(chart, windows),
            session: Session::new(scoring),
            scoring,
            level: None,
            inputs: Vec::new(),
            events: Vec::new(),
            clock_buf: Vec::new(),
            verdict_buf: Vec::new(),
            checkpoints: Vec::new(),
            current_segment: None,
            restarts: 0,
            mods: Mods::NONE,
            stars: 0.0,
            volley_charge: 0,
            marked: Vec::new(),
            next_cue: 0,
        }
    }

    pub fn mods(&self) -> Mods {
        self.mods
    }

    /// Chart difficulty (computed once, with mods applied).
    pub fn stars(&mut self) -> f64 {
        if self.stars <= 0.0 {
            self.stars = rating::rate(self.chart()).stars;
        }
        self.stars
    }

    pub fn volley_charge(&self) -> u32 {
        self.volley_charge
    }

    pub fn volley_ready(&self) -> bool {
        self.volley_charge >= VOLLEY_FULL
    }

    pub fn marked(&self) -> &[u32] {
        &self.marked
    }

    /// Mark a live weak point for the volley. Needs a full charge.
    pub fn mark(&mut self, target: u32) -> bool {
        if !self.volley_ready() || !self.judge.is_live(target) || self.marked.contains(&target) {
            return false;
        }
        if self.target(target).map(|t| t.kind) == Some(chart::Kind::Track) {
            return false;
        }
        self.inputs.push(Input::Mark { at: self.now_chart_time(), target });
        self.marked.push(target);
        true
    }

    /// Release the volley at a raw stream time. Judged against the nearest
    /// bar line; kills everything marked when on the beat. Either way the
    /// charge is spent and the marks are cleared.
    pub fn release(&mut self, raw_song_time: f64) -> Verdict {
        let t = self.chart_time(self.conductor.shot_time(raw_song_time));
        let at = t.max(self.now_chart_time());
        let restarts = self.restarts;
        self.verdict_buf.clear();
        self.judge.advance(at, &mut self.verdict_buf);
        self.apply_verdicts();
        if self.restarts != restarts {
            return Verdict::Ignored { target: u32::MAX, reason: "down".into() };
        }
        self.inputs.push(Input::Release { at, raw_time: raw_song_time });
        if !self.volley_ready() {
            self.marked.clear();
            return Verdict::VolleyMiss { delta_ms: self.judge.ms_from_nearest_bar(t), marked: 0 };
        }
        let marked = std::mem::take(&mut self.marked);
        self.verdict_buf.clear();
        let v = self.judge.fire_volley(t, &marked, &mut self.verdict_buf);
        for hit in self.verdict_buf.drain(..) {
            self.session.apply(&hit);
            self.events.push(GameEvent::Judge(hit));
        }
        self.session.apply(&v);
        self.volley_charge = 0;
        v
    }

    pub fn restarts(&self) -> u32 {
        self.restarts
    }

    /// Boss standing at the current chart time: the phase in progress (or the
    /// next one), kills and targets in it, kills and targets over the whole
    /// boss, and whether the boss is dead. `None` when the chart has no boss.
    pub fn boss_status(&self) -> Option<BossStatus> {
        let phases = &self.chart().phases;
        if phases.is_empty() {
            return None;
        }
        let now = self.now_chart_time();
        let mut current = phases.len() - 1;
        for (k, p) in phases.iter().enumerate() {
            if !self.judge.is_phase_closed(k) || now <= p.end_time {
                current = k;
                break;
            }
        }
        let (kills, total) = self.judge.phase_kills(current);
        let (mut all_kills, mut all_total) = (0, 0);
        for k in 0..phases.len() {
            let (a, b) = self.judge.phase_kills(k);
            all_kills += a;
            all_total += b;
        }
        let last = phases.len() - 1;
        let dead = self.judge.is_phase_closed(last) && {
            let (k, t) = self.judge.phase_kills(last);
            t == 0 || k as f64 >= (phases[last].clear * t as f64 - 1e-9).ceil()
        };
        Some(BossStatus { phase: current as u32, phases: phases.len() as u32, name: phases[current].name.clone(), kills, total, all_kills, all_total, phase_end_time: phases[current].end_time, dead })
    }

    /// Apply the judge's pending verdicts to the session and queue them as
    /// events. A `Down` triggers the section restart (unless `no_fail`):
    /// the rest of the batch belongs to the time that was rewound and is dropped.
    fn apply_verdicts(&mut self) {
        let advanced_to = self.judge.now();
        let batch: Vec<Verdict> = self.verdict_buf.drain(..).collect();
        for v in batch {
            if let Verdict::Spawn { target } = v {
                let seg = self.chart().targets[target as usize].segment;
                if self.current_segment != Some(seg) {
                    self.current_segment = Some(seg);
                    self.checkpoints.retain(|(s, _)| *s != seg);
                    self.checkpoints.push((seg, self.session.clone()));
                }
            }
            if let Verdict::Hit { timing, .. } = &v {
                if timing.is_on_beat() {
                    self.volley_charge = (self.volley_charge + 1).min(VOLLEY_FULL);
                } else {
                    self.volley_charge = 0;
                }
            }
            if matches!(v, Verdict::Expired { .. } | Verdict::Whiff { .. }) {
                self.volley_charge = 0;
                self.marked.clear();
            }
            let follow = self.session.apply(&v);
            // A checkpoint is taken at a segment's first spawn, which can come
            // before the previous segment has fully resolved (its last window,
            // a boss phase's close). Results that belong to an earlier segment
            // are applied to later checkpoints too, so a restore never loses a
            // cleared phase or a late miss of the section before.
            if let Some(seg) = self.verdict_segment(&v) {
                for (s, cp) in &mut self.checkpoints {
                    if *s > seg {
                        cp.apply(&v);
                    }
                }
            }
            self.events.push(GameEvent::Judge(v));
            if let Some(f) = follow {
                let is_down = matches!(f, Verdict::Down { .. });
                self.events.push(GameEvent::Judge(f));
                if is_down && !self.scoring.no_fail {
                    // The judge's time, not the conductor's: a click advances
                    // the judge past the conductor, and the replay must seek
                    // to where the judge was when it found the expiry.
                    self.inputs.push(Input::Advance { at: advanced_to });
                    self.restart_current_segment();
                    return;
                }
            }
        }
    }

    /// The segment a judged result belongs to, for checkpoint bookkeeping.
    fn verdict_segment(&self, v: &Verdict) -> Option<u32> {
        match v {
            Verdict::Hit { target, .. } | Verdict::Expired { target, .. } | Verdict::TrackDone { target, .. } => {
                self.chart().targets.get(*target as usize).map(|t| t.segment)
            }
            Verdict::PhaseEnd { segment, .. } => Some(*segment),
            _ => None,
        }
    }

    /// Rewind to one bar before the current segment's first enemy, restore
    /// the session checkpoint, reset the segment's targets.
    fn restart_current_segment(&mut self) {
        let Some(seg) = self.current_segment else { return };
        let Some(first_spawn) = self.judge.segment_first_spawn(seg) else { return };
        let bar = 4.0 * 60.0 / self.chart().bpm;
        let restart_time = first_spawn - bar;
        if let Some((_, cp)) = self.checkpoints.iter().find(|(s, _)| *s == seg) {
            let cp = cp.clone();
            self.session.restore(&cp);
        }
        self.judge.reset_segment(seg, restart_time);
        self.conductor.seek(restart_time + self.conductor.config().song_offset_s);
        self.restarts += 1;
        self.marked.clear();
        self.volley_charge = 0;
        self.next_cue = self.chart().cues.iter().position(|c| c.time >= restart_time).unwrap_or(self.chart().cues.len());
        self.events.push(GameEvent::Judge(Verdict::Restart { segment: seg, chart_time: restart_time }));
    }

    pub fn conductor(&self) -> &Conductor {
        &self.conductor
    }

    pub fn conductor_mut(&mut self) -> &mut Conductor {
        &mut self.conductor
    }

    pub fn judge(&self) -> &Judge {
        &self.judge
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn chart(&self) -> &Chart {
        self.judge.chart()
    }

    pub fn target(&self, id: u32) -> Option<&Target> {
        self.chart().targets.get(id as usize)
    }

    /// Chart time (seconds since beat 0) for a stream time.
    pub fn chart_time(&self, song_time: f64) -> f64 {
        song_time - self.conductor.config().song_offset_s
    }

    pub fn now_chart_time(&self) -> f64 {
        self.chart_time(self.conductor.song_time())
    }

    /// Start at a stream time. Use a negative value for a pre-roll so the
    /// player hears the count-in before beat 0.
    pub fn start(&mut self, song_time: f64) {
        self.conductor.start(song_time);
        self.inputs.clear();
        self.checkpoints.clear();
        self.current_segment = None;
        self.restarts = 0;
        self.volley_charge = 0;
        self.marked.clear();
        self.next_cue = 0;
        self.verdict_buf.clear();
        self.judge.advance(self.now_chart_time(), &mut self.verdict_buf);
        self.apply_verdicts();
        self.fire_cues();
    }

    /// Emit every cue whose bar the chart clock has reached.
    fn fire_cues(&mut self) {
        let now = self.now_chart_time();
        while self.next_cue < self.chart().cues.len() && self.chart().cues[self.next_cue].time <= now {
            self.events.push(GameEvent::Cue(self.next_cue));
            self.next_cue += 1;
        }
    }

    /// Frame step. Returns the events that happened during this step, in order.
    pub fn update(&mut self, dt: f64, audio_time: Option<f64>) -> std::vec::Drain<'_, GameEvent> {
        // Events left by fire()/seek since the last update stay at the front.
        self.clock_buf.clear();
        self.conductor.update(dt, audio_time, &mut self.clock_buf);
        for e in self.clock_buf.drain(..) {
            self.events.push(GameEvent::Clock(e));
        }
        self.verdict_buf.clear();
        self.judge.advance(self.now_chart_time(), &mut self.verdict_buf);
        self.apply_verdicts();
        self.fire_cues();
        self.events.drain(..)
    }

    /// Jump the model forward to a chart time without clock events (replay,
    /// scrubbing). Spawns and expiries up to that time are applied.
    pub fn seek_chart_time(&mut self, chart_time: f64) {
        let song_time = chart_time + self.conductor.config().song_offset_s;
        self.conductor.seek_forward(song_time);
        self.verdict_buf.clear();
        self.judge.advance(chart_time, &mut self.verdict_buf);
        self.apply_verdicts();
    }

    /// Advance the judge alone to a chart time (spawns, expiries, downs up to
    /// there), leaving the conductor on its clock. This is how a watched
    /// replay applies a recorded input at its recorded time while the song
    /// keeps playing: the judge is seeked to `at` exactly as `Replay::play`
    /// does, and the next `update` catches the clock up without re-judging.
    pub fn advance_judge(&mut self, chart_time: f64) {
        self.verdict_buf.clear();
        self.judge.advance(chart_time, &mut self.verdict_buf);
        self.apply_verdicts();
    }

    /// A click. `raw_song_time` is the stream time the engine measured for the
    /// input; calibration and offset are applied here. The judge is advanced
    /// to the click's own moment first, so the verdict does not depend on
    /// where the frame boundary fell; expiries that this reveals are emitted
    /// on the next `update`.
    pub fn fire(&mut self, raw_song_time: f64, hit: Option<Hit>) -> Verdict {
        let t = self.chart_time(self.conductor.shot_time(raw_song_time));
        let at = t.max(self.now_chart_time());
        let restarts = self.restarts;
        self.verdict_buf.clear();
        self.judge.advance(at, &mut self.verdict_buf);
        self.apply_verdicts();
        if self.restarts != restarts {
            // Advancing to this click found the expiry that downed the
            // player; the section has rewound and the click belongs to the
            // attempt that just ended. Not recorded, not judged.
            return Verdict::Ignored { target: hit.map(|h| h.target).unwrap_or(u32::MAX), reason: "down".into() };
        }
        self.inputs.push(Input::Shot {
            at,
            raw_time: raw_song_time,
            target: hit.map(|h| h.target as i64).unwrap_or(-1),
            offset_m: hit.map(|h| h.offset_m).unwrap_or(0.0),
        });
        let v = self.judge.fire(Shot { time: t, hit });
        match &v {
            Verdict::Hit { timing, .. } if timing.is_on_beat() => {
                self.volley_charge = (self.volley_charge + 1).min(VOLLEY_FULL);
            }
            Verdict::Hit { .. } | Verdict::Whiff { .. } => {
                self.volley_charge = 0;
                self.marked.clear();
            }
            _ => {}
        }
        self.session.apply(&v);
        v
    }

    pub fn track_sample(&mut self, target: u32, on: bool) -> bool {
        self.inputs.push(Input::Track { at: self.now_chart_time(), target, on });
        self.judge.track_sample(target, on)
    }

    pub fn level(&self) -> Option<&Level> {
        self.level.as_ref()
    }

    /// The run so far as a replay. `None` when the game was built from a bare
    /// chart rather than a level.
    pub fn replay(&self) -> Option<Replay> {
        Some(Replay {
            level: self.level.clone()?,
            clock: *self.conductor.config(),
            windows: self.judge.windows(),
            scoring: self.scoring,
            mods: self.mods,
            chart_fingerprint: self.chart().fingerprint(),
            inputs: self.inputs.clone(),
            end_chart_time: self.now_chart_time(),
        })
    }

    pub fn is_finished(&self) -> bool {
        self.judge.is_finished() && self.now_chart_time() >= self.chart().total_time
    }

    pub fn report(&self) -> Report {
        let stars = if self.stars > 0.0 { self.stars } else { rating::rate(self.chart()).stars };
        self.session.report(stars, self.mods)
    }
}
