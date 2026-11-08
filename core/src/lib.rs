//! HIGHCAL core: the engine-independent game model.
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
pub mod patterns;
pub mod rating;
pub mod replay;
pub mod score;
pub mod sim;

#[cfg(feature = "gdext")]
pub mod godot;

use chart::{Chart, Level, Target};
use clock::{ClockConfig, ClockEvent, Conductor};
use judge::{Hit, Judge, Shot, Verdict, Windows};
use replay::{Input, Replay};
use score::{Report, Scoring, Session};

#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    Clock(ClockEvent),
    Judge(Verdict),
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
}

impl Game {
    pub fn from_level(level: &Level, mut cfg: ClockConfig, windows: Windows, scoring: Scoring) -> Result<Self, chart::CompileError> {
        let chart = chart::compile(level, &patterns::builtin_lib())?;
        cfg.bpm = level.bpm;
        let mut g = Self::from_chart(chart, cfg, windows, scoring);
        g.level = Some(level.clone());
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
        }
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
        self.verdict_buf.clear();
        self.judge.advance(self.now_chart_time(), &mut self.verdict_buf);
        for v in self.verdict_buf.drain(..) {
            self.session.apply(&v);
            self.events.push(GameEvent::Judge(v));
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
        for v in self.verdict_buf.drain(..) {
            self.session.apply(&v);
            self.events.push(GameEvent::Judge(v));
        }
        self.events.drain(..)
    }

    /// Jump the model forward to a chart time without clock events (replay,
    /// scrubbing). Spawns and expiries up to that time are applied.
    pub fn seek_chart_time(&mut self, chart_time: f64) {
        let song_time = chart_time + self.conductor.config().song_offset_s;
        self.conductor.seek_forward(song_time);
        self.verdict_buf.clear();
        self.judge.advance(chart_time, &mut self.verdict_buf);
        for v in self.verdict_buf.drain(..) {
            self.session.apply(&v);
            self.events.push(GameEvent::Judge(v));
        }
    }

    /// A click. `raw_song_time` is the stream time the engine measured for the
    /// input; calibration and offset are applied here. The judge is advanced
    /// to the click's own moment first, so the verdict does not depend on
    /// where the frame boundary fell; expiries that this reveals are emitted
    /// on the next `update`.
    pub fn fire(&mut self, raw_song_time: f64, hit: Option<Hit>) -> Verdict {
        let t = self.chart_time(self.conductor.shot_time(raw_song_time));
        let at = t.max(self.now_chart_time());
        self.verdict_buf.clear();
        self.judge.advance(at, &mut self.verdict_buf);
        for v in self.verdict_buf.drain(..) {
            self.session.apply(&v);
            self.events.push(GameEvent::Judge(v));
        }
        self.inputs.push(Input::Shot {
            at,
            raw_time: raw_song_time,
            target: hit.map(|h| h.target as i64).unwrap_or(-1),
            offset_m: hit.map(|h| h.offset_m).unwrap_or(0.0),
        });
        let v = self.judge.fire(Shot { time: t, hit });
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
            chart_fingerprint: self.chart().fingerprint(),
            inputs: self.inputs.clone(),
        })
    }

    pub fn is_finished(&self) -> bool {
        self.judge.is_finished() && self.now_chart_time() >= self.chart().total_time
    }

    pub fn report(&self) -> Report {
        self.session.report()
    }
}
