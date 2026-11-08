//! Godot 4.7 bindings (gdext). `DbGame` wraps `Game`; GDScript talks to this
//! one object. Events come back as an Array of Dictionaries each frame so the
//! glue can route them to the HUD, spawner, VFX and music layers.

use crate::analysis::{analyze, auto_level, bar_energy, decode_audio, read_audio_mono, read_wav_mono};
use crate::chart::{Kind, Level};
use crate::clock::{ClockConfig, ClockEvent};
use crate::judge::{Hit, Verdict, Windows};
use crate::patterns::{drill_level, drills, fill_level, street_level, warmup_level};
use crate::score::Scoring;
use crate::{Game, GameEvent};
use godot::builtin::VarDictionary;
use godot::classes::RefCounted;
use godot::prelude::*;

struct DownbeatExtension;

#[gdextension]
unsafe impl ExtensionLibrary for DownbeatExtension {}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct DbGame {
    game: Option<Game>,
    no_fail: bool,
    mods: crate::mods::Mods,
    /// Inputs of the replay loaded by `load_replay`, for the watcher.
    replay_inputs: Vec<crate::replay::Input>,
    /// Last file analysis (path, analysis): nudging the grid re-reads this, not the file.
    analysis_cache: Option<(String, crate::analysis::Analysis)>,
    base: Base<RefCounted>,
}

fn kind_label(k: Kind) -> &'static str {
    k.label()
}

fn verdict_dict(v: &Verdict) -> VarDictionary {
    let mut d = VarDictionary::new();
    match v {
        Verdict::Spawn { target } => {
            d.set("type", "spawn");
            d.set("target", *target as i64);
        }
        Verdict::Hit { target, kind, timing, delta_ms, precision, hit_beat } => {
            d.set("type", "hit");
            d.set("target", *target as i64);
            d.set("kind", kind_label(*kind));
            d.set("timing", timing.label());
            d.set("on_beat", timing.is_on_beat());
            d.set("delta_ms", *delta_ms);
            d.set("precision", *precision as f64);
            d.set("hit_beat", *hit_beat);
        }
        Verdict::Whiff { grid_delta_ms } => {
            d.set("type", "whiff");
            d.set("delta_ms", *grid_delta_ms);
        }
        Verdict::Expired { target, kind } => {
            d.set("type", "expired");
            d.set("target", *target as i64);
            d.set("kind", kind_label(*kind));
        }
        Verdict::Damage { target, kind, amount, hp } => {
            d.set("type", "damage");
            d.set("target", *target as i64);
            d.set("kind", kind_label(*kind));
            d.set("amount", *amount as i64);
            d.set("hp", *hp as i64);
        }
        Verdict::Down { target, downs } => {
            d.set("type", "down");
            d.set("target", *target as i64);
            d.set("downs", *downs as i64);
        }
        Verdict::Restart { segment, chart_time } => {
            d.set("type", "restart");
            d.set("segment", *segment as i64);
            d.set("chart_time", *chart_time);
        }
        Verdict::Volley { timing, delta_ms, kills } => {
            d.set("type", "volley");
            d.set("timing", timing.label());
            d.set("delta_ms", *delta_ms);
            let mut arr: Array<i64> = Array::new();
            for k in kills {
                arr.push(*k as i64);
            }
            d.set("kills", &arr);
        }
        Verdict::VolleyMiss { delta_ms, marked } => {
            d.set("type", "volley_miss");
            d.set("delta_ms", *delta_ms);
            d.set("marked", *marked as i64);
        }
        Verdict::TrackDone { target, on, total } => {
            d.set("type", "track_done");
            d.set("target", *target as i64);
            d.set("on", *on as i64);
            d.set("total", *total as i64);
            d.set("ratio", if *total > 0 { *on as f64 / *total as f64 } else { 0.0 });
        }
        Verdict::Ignored { target, reason } => {
            d.set("type", "ignored");
            d.set("target", *target as i64);
            d.set("reason", reason.as_str());
        }
        Verdict::PhaseEnd { segment, phase, kills, total, cleared, last } => {
            d.set("type", "phase_end");
            d.set("segment", *segment as i64);
            d.set("phase", *phase as i64);
            d.set("kills", *kills as i64);
            d.set("total", *total as i64);
            d.set("cleared", *cleared);
            d.set("last", *last);
        }
    }
    d
}

fn clock_dict(e: &ClockEvent) -> VarDictionary {
    let mut d = VarDictionary::new();
    match e {
        ClockEvent::Tick { index, beat, sub } => {
            d.set("type", "tick");
            d.set("index", *index);
            d.set("beat", *beat);
            d.set("sub", *sub as i64);
        }
        ClockEvent::Beat { index, in_bar } => {
            d.set("type", "beat");
            d.set("index", *index);
            d.set("in_bar", *in_bar as i64);
        }
        ClockEvent::Bar { index } => {
            d.set("type", "bar");
            d.set("index", *index);
        }
    }
    d
}

fn analysis_dict(a: &crate::analysis::Analysis) -> VarDictionary {
    let mut d = VarDictionary::new();
    d.set("bpm", a.bpm);
    d.set("offset_s", a.offset_s);
    d.set("tempo_confidence", a.tempo_confidence);
    d.set("phase_confidence", a.phase_confidence);
    d.set("beats", a.beats);
    d.set("bars", a.bars);
    d.set("duration_s", a.duration_s);
    d.set("sample_rate", a.sample_rate as i64);
    d.set("onset_grid", &onset_grid(a, a.bpm, a.offset_s));
    d
}

/// 64 sixteenth slots from beat 0 (four bars), each the peak onset level 0..1
/// around that slot: the confirm screen draws these over the grid.
fn onset_grid(a: &crate::analysis::Analysis, bpm: f64, offset_s: f64) -> Array<f64> {
    use crate::analysis::{HOP, ONSET_LAG_SAMPLES};
    let mut out: Array<f64> = Array::new();
    let peak = a.onset.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    let spb = 60.0 / bpm.max(1.0);
    for k in 0..64 {
        let t = offset_s + k as f64 * spb / 4.0;
        let centre = ((t * a.sample_rate as f64 - ONSET_LAG_SAMPLES) / HOP as f64).round().max(0.0) as usize;
        let v = (centre.saturating_sub(1)..=centre + 1)
            .filter_map(|i| a.onset.get(i).copied())
            .fold(0.0f32, f32::max)
            / peak;
        out.push(v as f64);
    }
    out
}

fn report_dict(r: &crate::score::Report) -> VarDictionary {
    let mut d = VarDictionary::new();
    d.set("score", r.score as i64);
    d.set("raw_score", r.raw_score as i64);
    d.set("mods", r.mods.as_str());
    d.set("mod_multiplier", r.mod_multiplier);
    d.set("stars", r.stars);
    d.set("pp", r.pp);
    d.set("fc", r.fc);
    d.set("pfc", r.pfc);
    d.set("hp", r.hp as i64);
    d.set("damage_taken", r.damage_taken as i64);
    d.set("downs", r.downs as i64);
    d.set("phases_cleared", r.phases_cleared as i64);
    d.set("phase_fails", r.phase_fails as i64);
    d.set("max_combo", r.max_combo as i64);
    d.set("shots", r.shots as i64);
    d.set("hits", r.hits as i64);
    d.set("whiffs", r.whiffs as i64);
    d.set("expired", r.expired as i64);
    d.set("perfect", r.perfect as i64);
    d.set("great", r.great as i64);
    d.set("good", r.good as i64);
    d.set("early", r.early as i64);
    d.set("late", r.late as i64);
    d.set("aim_accuracy", r.aim_accuracy);
    d.set("precision", r.precision);
    d.set("beat_accuracy", r.beat_accuracy);
    d.set("timing_quality", r.timing_quality);
    d.set("timing_mean_ms", r.timing_mean_ms);
    d.set("timing_std_ms", r.timing_std_ms);
    d.set("track_ratio", r.track_ratio);
    d.set("composite", r.composite);
    d.set("grade", r.grade.label());
    let mut kinds = VarDictionary::new();
    for (k, s) in &r.by_kind {
        let mut kd = VarDictionary::new();
        kd.set("targets", s.targets as i64);
        kd.set("hits", s.hits as i64);
        kd.set("on_beat", s.on_beat as i64);
        kd.set("expired", s.expired as i64);
        kd.set("precision", if s.hits > 0 { s.precision_sum / s.hits as f64 } else { 0.0 });
        kd.set("track_on", s.track_on as i64);
        kd.set("track_total", s.track_total as i64);
        kinds.set(k.as_str(), &kd);
    }
    d.set("by_kind", &kinds);
    let mut history: Array<VarDictionary> = Array::new();
    for h in &r.history {
        let mut hd = VarDictionary::new();
        hd.set("target", h.target as i64);
        hd.set("kind", kind_label(h.kind));
        hd.set("timing", h.timing.label());
        hd.set("hit_beat", h.hit_beat);
        hd.set("delta_ms", h.delta_ms);
        hd.set("precision", h.precision as f64);
        history.push(&hd);
    }
    d.set("history", &history);
    d
}

#[godot_api]
impl IRefCounted for DbGame {
    fn init(base: Base<RefCounted>) -> Self {
        Self { game: None, no_fail: false, mods: crate::mods::Mods::NONE, replay_inputs: Vec::new(), analysis_cache: None, base }
    }
}

#[godot_api]
impl DbGame {
    /// Builtin warmup chart at any BPM.
    #[func]
    fn load_warmup(&mut self, bpm: f64, song_offset_s: f64, input_offset_ms: f64) -> bool {
        self.load_level(&warmup_level(bpm), song_offset_s, input_offset_ms)
    }

    /// Builtin fill chart (every kind including RUSH) at any BPM.
    #[func]
    fn load_fill(&mut self, bpm: f64, song_offset_s: f64, input_offset_ms: f64) -> bool {
        self.load_level(&fill_level(bpm), song_offset_s, input_offset_ms)
    }

    /// A level authored as JSON (see levels/*.json).
    #[func]
    fn load_level_json(&mut self, json: GString, song_offset_s: f64, input_offset_ms: f64) -> bool {
        match Level::from_json(&json.to_string()) {
            Ok(level) => self.load_level(&level, song_offset_s, input_offset_ms),
            Err(e) => {
                godot_error!("downbeat: level json: {e}");
                false
            }
        }
    }

    #[func]
    fn warmup_level_json(bpm: f64) -> GString {
        GString::from(warmup_level(bpm).to_json_pretty().as_str())
    }

    /// Audio path from the loaded level ("" = procedural click).
    #[func]
    fn audio_path(&self) -> GString {
        GString::from(self.level_field(|l| l.audio.clone()).unwrap_or_default().as_str())
    }

    /// Stem paths from the loaded level.
    #[func]
    fn stem_paths(&self) -> Array<GString> {
        let mut out = Array::new();
        for p in self.level_field(|l| l.stems.clone()).unwrap_or_default() {
            out.push(&GString::from(p.as_str()));
        }
        out
    }

    /// Builtin act-one level: street -> atrium -> platform.
    #[func]
    fn load_street(&mut self, bpm: f64, song_offset_s: f64, input_offset_ms: f64) -> bool {
        self.load_level(&street_level(bpm), song_offset_s, input_offset_ms)
    }

    /// The Range drill catalogue: name, skill, description, pass_beats.
    #[func]
    fn drills(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let lib = crate::patterns::builtin_lib();
        for d in drills() {
            let mut dd = VarDictionary::new();
            dd.set("name", d.name.as_str());
            dd.set("skill", d.skill.as_str());
            dd.set("description", d.description.as_str());
            let beats: f64 = d.sections.iter().map(|s| lib.get(&s.pattern).map(|p| p.length * s.repeats as f64).unwrap_or(4.0)).sum();
            dd.set("pass_beats", beats);
            out.push(&dd);
        }
        out
    }

    /// Load a Range drill at a BPM for about `minutes`.
    #[func]
    fn load_drill(&mut self, name: GString, bpm: f64, minutes: f64, input_offset_ms: f64) -> bool {
        match drill_level(&name.to_string(), bpm, minutes) {
            Some(level) => self.load_level(&level, 0.0, input_offset_ms),
            None => {
                godot_error!("downbeat: unknown drill {name}");
                false
            }
        }
    }

    #[func]
    fn rank_for(&self, mean_composite: f64) -> GString {
        GString::from(crate::rating::rank(mean_composite))
    }

    #[func]
    fn bpm_band(&self, bpm: f64) -> GString {
        GString::from(crate::rating::bpm_band(bpm).as_str())
    }

    /// Rooms with resolved origins: name, origin, size, stand, yaw_deg, blocks [{pos,size}].
    #[func]
    fn rooms(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(g) = &self.game else { return out };
        for r in &g.chart().rooms {
            let mut d = VarDictionary::new();
            d.set("name", r.name.as_str());
            let o = r.origin.unwrap_or([0.0; 3]);
            d.set("origin", Vector3::new(o[0], o[1], o[2]));
            d.set("size", Vector3::new(r.size[0], r.size[1], r.size[2]));
            let sw = r.stand_world();
            d.set("stand", Vector3::new(sw[0], sw[1], sw[2]));
            d.set("yaw_deg", r.yaw_deg as f64);
            let mut blocks: Array<VarDictionary> = Array::new();
            for b in &r.blocks {
                let mut bd = VarDictionary::new();
                bd.set("pos", Vector3::new(o[0] + b.pos[0], o[1] + b.pos[1], o[2] + b.pos[2]));
                bd.set("size", Vector3::new(b.size[0], b.size[1], b.size[2]));
                blocks.push(&bd);
            }
            d.set("blocks", &blocks);
            out.push(&d);
        }
        out
    }

    /// Segment spans in path order: index, kind (traversal|encounter), room, start/end beat and time.
    #[func]
    fn spans(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(g) = &self.game else { return out };
        for s in &g.chart().spans {
            let mut d = VarDictionary::new();
            d.set("index", s.index as i64);
            d.set("kind", s.kind.as_str());
            d.set("room", s.room.as_str());
            d.set("start_beat", s.start_beat);
            d.set("end_beat", s.end_beat);
            d.set("start_time", s.start_time);
            d.set("end_time", s.end_time);
            d.set("phase", s.phase.map(|p| p as i64).unwrap_or(-1));
            d.set("label", s.label.as_str());
            out.push(&d);
        }
        out
    }

    /// Boss phases of the chart: {segment, phase, name, room, start/end beat and time, targets, clear, last}.
    #[func]
    fn phases(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(g) = &self.game else { return out };
        for p in &g.chart().phases {
            let mut d = VarDictionary::new();
            d.set("segment", p.segment as i64);
            d.set("phase", p.phase as i64);
            d.set("name", p.name.as_str());
            d.set("room", p.room.as_str());
            d.set("start_beat", p.start_beat);
            d.set("end_beat", p.end_beat);
            d.set("start_time", p.start_time);
            d.set("end_time", p.end_time);
            d.set("targets", p.targets as i64);
            d.set("clear", p.clear);
            d.set("last", p.last);
            out.push(&d);
        }
        out
    }

    /// The boss right now: {phase, phases, name, kills, total, all_kills, all_total, phase_end_time, dead}; empty without a boss.
    #[func]
    fn boss_status(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(b) = self.game.as_ref().and_then(|g| g.boss_status()) else { return d };
        d.set("phase", b.phase as i64);
        d.set("phases", b.phases as i64);
        d.set("name", b.name.as_str());
        d.set("kills", b.kills as i64);
        d.set("total", b.total as i64);
        d.set("all_kills", b.all_kills as i64);
        d.set("all_total", b.all_total as i64);
        d.set("phase_end_time", b.phase_end_time);
        d.set("dead", b.dead);
        d
    }

    /// Set pieces of the chart in time order: {kind, room, beat, time, bars, param}.
    #[func]
    fn cues(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(g) = &self.game else { return out };
        for c in &g.chart().cues {
            let mut d = VarDictionary::new();
            d.set("kind", c.kind.as_str());
            d.set("room", c.room.as_str());
            d.set("beat", c.beat);
            d.set("time", c.time);
            d.set("bars", c.bars);
            d.set("param", c.param);
            out.push(&d);
        }
        out
    }

    #[func]
    fn restarts(&self) -> i64 {
        self.game.as_ref().map(|g| g.restarts() as i64).unwrap_or(0)
    }

    /// NF mod: downs reset HP without restarting the section. Takes effect on the next load.
    #[func]
    fn set_no_fail(&mut self, on: bool) {
        self.no_fail = on;
    }

    /// Mods for the next load, as a label: "DT", "HR", "DTHRHD", "NM"...
    #[func]
    fn set_mods(&mut self, label: GString) {
        self.mods = crate::mods::Mods::from_label(&label.to_string());
    }

    #[func]
    fn mods(&self) -> GString {
        GString::from(self.game.as_ref().map(|g| g.mods()).unwrap_or(self.mods).label().as_str())
    }

    #[func]
    fn mod_multiplier(&self) -> f64 {
        self.mods.multiplier()
    }

    /// Downbeat volley: charge 0..16, ready flag, marked ids.
    #[func]
    fn volley_charge(&self) -> i64 {
        self.game.as_ref().map(|g| g.volley_charge() as i64).unwrap_or(0)
    }

    #[func]
    fn volley_ready(&self) -> bool {
        self.game.as_ref().map(|g| g.volley_ready()).unwrap_or(false)
    }

    #[func]
    fn mark(&mut self, target: i64) -> bool {
        match &mut self.game {
            Some(g) if target >= 0 => g.mark(target as u32),
            _ => false,
        }
    }

    #[func]
    fn marked(&self) -> Array<i64> {
        let mut out = Array::new();
        if let Some(g) = &self.game {
            for m in g.marked() {
                out.push(*m as i64);
            }
        }
        out
    }

    #[func]
    fn release(&mut self, raw_song_time: f64) -> VarDictionary {
        match &mut self.game {
            Some(g) => verdict_dict(&g.release(raw_song_time)),
            None => VarDictionary::new(),
        }
    }

    /// Fair-play inspection of the current run: flags[], suspicion, hits_checked.
    #[func]
    fn integrity(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(g) = &self.game else { return d };
        let Some(replay) = g.replay() else { return d };
        let i = crate::integrity::inspect(&g.report(), &replay);
        let mut flags: Array<GString> = Array::new();
        for f in &i.flags {
            flags.push(&GString::from(f.as_str()));
        }
        d.set("flags", &flags);
        d.set("suspicion", i.suspicion);
        d.set("hits_checked", i.hits_checked as i64);
        d.set("clean", i.clean());
        d
    }

    /// Load a saved replay to watch it: the game is rebuilt from the replay's
    /// level, mods and clock settings (calibration included), so feeding the
    /// recorded inputs back through `fire`/`track_sample`/`mark`/`release`
    /// at their `at` times reproduces the run. The inputs are kept for
    /// `replay_inputs`.
    #[func]
    fn load_replay(&mut self, json: GString) -> bool {
        let parsed = match crate::replay::Replay::from_json(&json.to_string()) {
            Ok(r) => r,
            Err(e) => {
                godot_error!("downbeat: replay json: {e}");
                return false;
            }
        };
        match Game::from_level_with_mods(&parsed.level, parsed.mods, parsed.clock, parsed.windows, parsed.scoring) {
            Ok(g) => {
                if g.chart().fingerprint() != parsed.chart_fingerprint {
                    godot_error!("downbeat: replay: chart changed since the run");
                    return false;
                }
                self.mods = parsed.mods;
                self.no_fail = parsed.scoring.no_fail;
                self.replay_inputs = parsed.inputs;
                self.game = Some(g);
                true
            }
            Err(e) => {
                godot_error!("downbeat: replay: {e}");
                false
            }
        }
    }

    /// The loaded replay's inputs in order: {type, at, raw_time, target, offset_m, on}.
    #[func]
    fn replay_inputs(&self) -> Array<VarDictionary> {
        use crate::replay::Input;
        let mut out = Array::new();
        for i in &self.replay_inputs {
            let mut d = VarDictionary::new();
            match *i {
                Input::Shot { at, raw_time, target, offset_m } => {
                    d.set("type", "shot");
                    d.set("at", at);
                    d.set("raw_time", raw_time);
                    d.set("target", target);
                    d.set("offset_m", offset_m as f64);
                }
                Input::Track { at, target, on } => {
                    d.set("type", "track");
                    d.set("at", at);
                    d.set("target", target as i64);
                    d.set("on", on);
                }
                Input::Advance { at } => {
                    d.set("type", "advance");
                    d.set("at", at);
                }
                Input::Mark { at, target } => {
                    d.set("type", "mark");
                    d.set("at", at);
                    d.set("target", target as i64);
                }
                Input::Release { at, raw_time } => {
                    d.set("type", "release");
                    d.set("at", at);
                    d.set("raw_time", raw_time);
                }
            }
            out.push(&d);
        }
        out
    }

    /// Decode an audio file (wav, flac, ogg, mp3, m4a) to 16-bit PCM for an
    /// engine stream: {sample_rate, channels (1|2), pcm: PackedByteArray,
    /// duration_s}. Empty dictionary with an error printed on failure.
    #[func]
    fn decode_file(&self, path: GString) -> VarDictionary {
        let mut d = VarDictionary::new();
        match decode_audio(&path.to_string()) {
            Ok(dec) => {
                let (bytes, ch) = dec.pcm16();
                let frames = bytes.len() / (2 * ch.max(1));
                d.set("sample_rate", dec.sample_rate as i64);
                d.set("channels", ch as i64);
                d.set("duration_s", frames as f64 / dec.sample_rate.max(1) as f64);
                d.set("pcm", &PackedByteArray::from(bytes.as_slice()));
            }
            Err(e) => godot_error!("downbeat: decode: {e}"),
        }
        d
    }

    /// Profile rating from best performances (one per chart), osu-weighted.
    #[func]
    fn profile_rating(&self, best_pp: Array<f64>) -> f64 {
        let v: Vec<f64> = best_pp.iter_shared().collect();
        crate::rating::profile_rating(&v)
    }

    /// Song metadata of the loaded level: song, artist, difficulty, charter.
    #[func]
    fn level_meta(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(l) = self.game.as_ref().and_then(|g| g.level()) {
            d.set("name", l.name.as_str());
            d.set("song", l.song.as_str());
            d.set("artist", l.artist.as_str());
            d.set("difficulty", l.difficulty.as_str());
            d.set("charter", l.charter.as_str());
        }
        d
    }

    /// Street, Hard.
    #[func]
    fn load_street_hard(&mut self, bpm: f64, song_offset_s: f64, input_offset_ms: f64) -> bool {
        self.load_level(&crate::patterns::street_hard_level(bpm), song_offset_s, input_offset_ms)
    }

    #[func]
    fn hp(&self) -> i64 {
        self.game.as_ref().map(|g| g.session().hp() as i64).unwrap_or(0)
    }

    /// Kit sample paths by kind label; empty means use the synthesised kit.
    #[func]
    fn kit_paths(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        for (k, v) in self.level_field(|l| l.kit.clone()).unwrap_or_default() {
            d.set(k.as_str(), v.as_str());
        }
        d
    }

    #[func]
    fn song_offset_s(&self) -> f64 {
        self.level_field(|l| l.song_offset_s).unwrap_or(0.0)
    }

    #[func]
    fn level_name(&self) -> GString {
        GString::from(self.level_field(|l| l.name.clone()).unwrap_or_default().as_str())
    }

    /// The run so far as replay JSON (empty if no level is loaded).
    #[func]
    fn replay_json(&self) -> GString {
        match self.game.as_ref().and_then(|g| g.replay()) {
            Some(r) => GString::from(r.to_json().as_str()),
            None => GString::new(),
        }
    }

    /// Replay a saved run and return its report (empty dictionary on error).
    #[func]
    fn replay_report(&self, json: GString) -> VarDictionary {
        let parsed = match crate::replay::Replay::from_json(&json.to_string()) {
            Ok(r) => r,
            Err(e) => {
                godot_error!("downbeat: replay json: {e}");
                return VarDictionary::new();
            }
        };
        match parsed.play() {
            Ok(rep) => report_dict(&rep),
            Err(e) => {
                godot_error!("downbeat: replay: {e}");
                VarDictionary::new()
            }
        }
    }

    /// Tempo / beat-0 analysis of a WAV on disk (absolute path). Keys: bpm,
    /// offset_s, tempo_confidence, phase_confidence, beats, bars, duration_s.
    #[func]
    fn analyze_wav(&self, path: GString) -> VarDictionary {
        match read_wav_mono(&path.to_string()) {
            Ok((samples, sr)) => analysis_dict(&analyze(&samples, sr)),
            Err(e) => {
                godot_error!("downbeat: analyze: {e}");
                VarDictionary::new()
            }
        }
    }

    /// Analysis of any supported audio file (wav, flac, ogg, mp3, m4a) at an
    /// absolute path. Adds `energy`: normalised onset energy per bar on the
    /// detected grid, and `onset_grid`: 64 sixteenth-slot levels from beat 0.
    #[func]
    fn analyze_file(&mut self, path: GString) -> VarDictionary {
        let p = path.to_string();
        match read_audio_mono(&p) {
            Ok((samples, sr)) => {
                let a = analyze(&samples, sr);
                let mut d = analysis_dict(&a);
                let bars = (((a.duration_s - a.offset_s) / (60.0 / a.bpm.max(1.0))) / 4.0).floor().max(0.0) as usize;
                let energy = bar_energy(&a, a.bpm, a.offset_s, bars);
                let mut e: Array<f64> = Array::new();
                for v in energy {
                    e.push(v);
                }
                d.set("energy", &e);
                d.set("bars", bars as i64);
                d.set("onset_grid", &onset_grid(&a, a.bpm, a.offset_s));
                self.analysis_cache = Some((p, a));
                d
            }
            Err(e) => {
                godot_error!("downbeat: analyze_file: {e}");
                VarDictionary::new()
            }
        }
    }

    /// Re-run the grid readout for a corrected tempo/offset (user nudges):
    /// energy per bar and the onset grid. Uses the cached analysis of the
    /// last `analyze_file` call for the same path, so a nudge is instant.
    #[func]
    fn regrid_file(&mut self, path: GString, bpm: f64, offset_s: f64) -> VarDictionary {
        let p = path.to_string();
        let cached = matches!(&self.analysis_cache, Some((cp, _)) if *cp == p);
        if !cached {
            match read_audio_mono(&p) {
                Ok((samples, sr)) => {
                    let a = analyze(&samples, sr);
                    self.analysis_cache = Some((p.clone(), a));
                }
                Err(e) => {
                    godot_error!("downbeat: regrid: {e}");
                    return VarDictionary::new();
                }
            }
        }
        let mut d = VarDictionary::new();
        let Some((_, a)) = &self.analysis_cache else { return d };
        let bars = (((a.duration_s - offset_s) / (60.0 / bpm.max(1.0))) / 4.0).floor().max(0.0) as usize;
        let mut e: Array<f64> = Array::new();
        for v in bar_energy(a, bpm, offset_s, bars) {
            e.push(v);
        }
        d.set("energy", &e);
        d.set("bars", bars as i64);
        d.set("onset_grid", &onset_grid(a, bpm, offset_s));
        d.set("duration_s", a.duration_s);
        d
    }

    /// A metronome rendered onto a song's grid: mono 16-bit PCM with a
    /// downbeat tone on every bar and a quarter tone on every beat from
    /// `offset_s` to `duration_s`. Played sample-locked next to the song so a
    /// wrong tempo or offset is heard.
    #[func]
    fn grid_click(&self, bpm: f64, offset_s: f64, duration_s: f64, sample_rate: i64) -> VarDictionary {
        let sr = sample_rate.max(8000) as u32;
        let bytes = crate::analysis::click_grid_pcm16(bpm, offset_s, duration_s, sr);
        let mut d = VarDictionary::new();
        d.set("sample_rate", sr as i64);
        d.set("channels", 1i64);
        d.set("pcm", &PackedByteArray::from(bytes.as_slice()));
        d
    }

    /// Build a playable level for a song: energy-mapped encounters in the
    /// Range room, the file as audio, the measured grid.
    #[func]
    fn auto_level_json(&self, name: GString, audio: GString, bpm: f64, offset_s: f64, duration_s: f64, energy: Array<f64>) -> GString {
        let e: Vec<f64> = energy.iter_shared().collect();
        let level = auto_level(&name.to_string(), &audio.to_string(), bpm, offset_s, duration_s, &e);
        GString::from(level.to_json_pretty().as_str())
    }

    /// Watching: advance the judge alone to a recorded input's time (see `Game::advance_judge`).
    #[func]
    fn advance_judge(&mut self, chart_time: f64) {
        if let Some(g) = &mut self.game {
            g.advance_judge(chart_time);
        }
    }

    #[func]
    fn start(&mut self, song_time: f64) {
        if let Some(g) = &mut self.game {
            g.start(song_time);
        }
    }

    /// Frame step. `has_audio` false = open loop (menus, pre-roll before the
    /// stream starts). Returns this frame's events in order.
    #[func]
    fn update(&mut self, dt: f64, has_audio: bool, audio_time: f64) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(g) = &mut self.game else { return out };
        let audio = if has_audio { Some(audio_time) } else { None };
        let events: Vec<GameEvent> = g.update(dt, audio).collect();
        for e in &events {
            match e {
                GameEvent::Clock(c) => out.push(&clock_dict(c)),
                GameEvent::Judge(v) => out.push(&verdict_dict(v)),
                GameEvent::Cue(i) => {
                    let c = &g.chart().cues[*i];
                    let mut d = VarDictionary::new();
                    d.set("type", "cue");
                    d.set("kind", c.kind.as_str());
                    d.set("room", c.room.as_str());
                    d.set("beat", c.beat);
                    d.set("time", c.time);
                    d.set("bars", c.bars);
                    d.set("param", c.param);
                    out.push(&d);
                }
            }
        }
        out
    }

    /// A click. `target` < 0 = the ray hit nothing judgeable.
    #[func]
    fn fire(&mut self, raw_song_time: f64, target: i64, offset_m: f64) -> VarDictionary {
        let Some(g) = &mut self.game else { return VarDictionary::new() };
        let hit = if target >= 0 { Some(Hit { target: target as u32, offset_m: offset_m as f32 }) } else { None };
        verdict_dict(&g.fire(raw_song_time, hit))
    }

    #[func]
    fn track_sample(&mut self, target: i64, on: bool) -> bool {
        match &mut self.game {
            Some(g) if target >= 0 => g.track_sample(target as u32, on),
            _ => false,
        }
    }

    #[func]
    fn set_input_offset_ms(&mut self, ms: f64) {
        if let Some(g) = &mut self.game {
            g.conductor_mut().set_input_offset_ms(ms);
        }
    }

    #[func]
    fn bpm(&self) -> f64 {
        self.game.as_ref().map(|g| g.conductor().config().bpm).unwrap_or(0.0)
    }

    #[func]
    fn sec_per_beat(&self) -> f64 {
        self.game.as_ref().map(|g| g.conductor().sec_per_beat()).unwrap_or(0.0)
    }

    #[func]
    fn song_time(&self) -> f64 {
        self.game.as_ref().map(|g| g.conductor().song_time()).unwrap_or(0.0)
    }

    #[func]
    fn song_beat(&self) -> f64 {
        self.game.as_ref().map(|g| g.conductor().song_beat()).unwrap_or(0.0)
    }

    #[func]
    fn beat_phase(&self) -> f64 {
        self.game.as_ref().map(|g| g.conductor().beat_phase()).unwrap_or(0.0)
    }

    #[func]
    fn drift_ms(&self) -> f64 {
        self.game.as_ref().map(|g| g.conductor().drift_ms()).unwrap_or(0.0)
    }

    #[func]
    fn chart_time(&self, song_time: f64) -> f64 {
        self.game.as_ref().map(|g| g.chart_time(song_time)).unwrap_or(0.0)
    }

    #[func]
    fn chart_total_time(&self) -> f64 {
        self.game.as_ref().map(|g| g.chart().total_time).unwrap_or(0.0)
    }

    /// Difficulty of the loaded chart: stars, aim_speed, precision, density, tracking.
    #[func]
    fn rating(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(g) = &self.game else { return d };
        let r = crate::rating::rate(g.chart());
        d.set("stars", r.stars);
        d.set("aim_speed", r.aim_speed);
        d.set("precision", r.precision);
        d.set("density", r.density);
        d.set("tracking", r.tracking);
        d
    }

    /// Rate a level JSON without loading it: stars, bpm, bars, targets, name,
    /// fingerprint. For level lists.
    #[func]
    fn rate_level_json(&self, json: GString) -> VarDictionary {
        self.rate_level_json_mods(json, GString::from("NM"))
    }

    /// Rating, fingerprint and metadata of a level JSON under a mod set, the
    /// way a run with those mods would be keyed on the leaderboard.
    #[func]
    fn rate_level_json_mods(&self, json: GString, mods: GString) -> VarDictionary {
        let mut d = VarDictionary::new();
        let level = match Level::from_json(&json.to_string()) {
            Ok(l) => l,
            Err(e) => {
                godot_error!("downbeat: level json: {e}");
                return d;
            }
        };
        let m = crate::mods::Mods::from_label(&mods.to_string());
        let mut modded = level.clone();
        modded.bpm *= m.bpm_factor();
        match crate::chart::compile(&modded, &crate::patterns::builtin_lib()) {
            Ok(mut chart) => {
                if m.hr {
                    for t in &mut chart.targets {
                        t.radius *= m.radius_factor();
                    }
                }
                let r = crate::rating::rate(&chart);
                d.set("name", level.name.as_str());
                d.set("bpm", level.bpm);
                d.set("stars", r.stars);
                d.set("aim_speed", r.aim_speed);
                d.set("density", r.density);
                d.set("tracking", r.tracking);
                d.set("bars", (chart.total_beats / 4.0).floor());
                d.set("targets", chart.targets.len() as i64);
                d.set("duration_s", chart.total_time);
                d.set("audio", level.audio.as_str());
                d.set("stems", level.stems.len() as i64);
                d.set("song", level.song.as_str());
                d.set("artist", level.artist.as_str());
                d.set("difficulty", level.difficulty.as_str());
                d.set("charter", level.charter.as_str());
                d.set("mods", m.label().as_str());
                d.set("fingerprint", format!("{:016x}", chart.fingerprint()).as_str());
            }
            Err(e) => godot_error!("downbeat: compile: {e}"),
        }
        d
    }

    /// A builtin level ("street", "street_hard", "warmup", "fill") as JSON at a BPM; empty for an unknown name.
    #[func]
    fn builtin_level_json(name: GString, bpm: f64) -> GString {
        let level = match name.to_string().as_str() {
            "street" => street_level(bpm),
            "street_hard" => crate::patterns::street_hard_level(bpm),
            "warmup" => warmup_level(bpm),
            "fill" => fill_level(bpm),
            _ => return GString::new(),
        };
        GString::from(level.to_json_pretty().as_str())
    }

    #[func]
    fn chart_fingerprint(&self) -> GString {
        match &self.game {
            Some(g) => GString::from(format!("{:016x}", g.chart().fingerprint()).as_str()),
            None => GString::new(),
        }
    }

    #[func]
    fn target_count(&self) -> i64 {
        self.game.as_ref().map(|g| g.chart().targets.len() as i64).unwrap_or(0)
    }

    #[func]
    fn is_finished(&self) -> bool {
        self.game.as_ref().map(|g| g.is_finished()).unwrap_or(true)
    }

    #[func]
    fn combo(&self) -> i64 {
        self.game.as_ref().map(|g| g.session().combo() as i64).unwrap_or(0)
    }

    #[func]
    fn score(&self) -> i64 {
        self.game.as_ref().map(|g| g.session().score() as i64).unwrap_or(0)
    }

    /// Static geometry of a target. Angles in degrees from the lane origin.
    #[func]
    fn target(&self, id: i64) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(t) = self.game.as_ref().and_then(|g| g.target(id.max(0) as u32)) else { return d };
        d.set("id", t.id as i64);
        d.set("kind", kind_label(t.kind));
        d.set("yaw", t.aim.yaw as f64);
        d.set("pitch", t.aim.pitch as f64);
        d.set("dist", t.aim.dist as f64);
        d.set("radius", t.radius as f64);
        d.set("hit_time", t.hit_time);
        d.set("spawn_time", t.spawn_time);
        d.set("end_time", t.end_time);
        d.set("hit_beat", t.hit_beat);
        d.set("phrase", t.phrase as i64);
        d.set("pattern", t.pattern.as_str());
        d.set("room", t.room.as_str());
        d.set("segment", t.segment as i64);
        d.set("phase", t.phase.map(|p| p as i64).unwrap_or(-1));
        d.set("enemy", t.kind.enemy());
        d.set("damage", t.kind.damage() as i64);
        d.set("pos", Vector3::new(t.pos[0], t.pos[1], t.pos[2]));
        if let Some(tp) = t.to_pos {
            d.set("to_pos", Vector3::new(tp[0], tp[1], tp[2]));
        }
        if let Some(to) = t.to {
            d.set("to_yaw", to.yaw as f64);
            d.set("to_pitch", to.pitch as f64);
            d.set("to_dist", to.dist as f64);
        }
        d
    }

    /// World position of a target's weak point at a chart time (metres, Y up).
    /// Striders move along their path; everything else is fixed.
    #[func]
    fn target_position_at(&self, id: i64, chart_time: f64) -> Vector3 {
        match self.game.as_ref().and_then(|g| g.target(id.max(0) as u32)) {
            Some(t) => {
                let p = t.pos_at(chart_time);
                Vector3::new(p[0], p[1], p[2])
            }
            None => Vector3::ZERO,
        }
    }

    #[func]
    fn report(&self) -> VarDictionary {
        match &self.game {
            Some(g) => report_dict(&g.report()),
            None => VarDictionary::new(),
        }
    }

    fn level_field<T>(&self, f: impl Fn(&Level) -> T) -> Option<T> {
        self.game.as_ref().and_then(|g| g.level().map(f))
    }

    fn load_level(&mut self, level: &Level, song_offset_s: f64, input_offset_ms: f64) -> bool {
        let cfg = ClockConfig { bpm: level.bpm, song_offset_s, input_offset_ms, ..ClockConfig::default() };
        let scoring = Scoring { no_fail: self.no_fail, ..Scoring::default() };
        match Game::from_level_with_mods(level, self.mods, cfg, Windows::default(), scoring) {
            Ok(g) => {
                self.game = Some(g);
                true
            }
            Err(e) => {
                godot_error!("downbeat: compile: {e}");
                false
            }
        }
    }
}
