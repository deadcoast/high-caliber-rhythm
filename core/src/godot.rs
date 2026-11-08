//! Godot 4.7 bindings (gdext). `DbGame` wraps `Game`; GDScript talks to this
//! one object. Events come back as an Array of Dictionaries each frame so the
//! glue can route them to the HUD, spawner, VFX and music layers.

use crate::analysis::{analyze, read_wav_mono};
use crate::chart::{Kind, Level};
use crate::clock::{ClockConfig, ClockEvent};
use crate::judge::{Hit, Verdict, Windows};
use crate::patterns::{fill_level, warmup_level};
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

fn report_dict(r: &crate::score::Report) -> VarDictionary {
    let mut d = VarDictionary::new();
    d.set("score", r.score as i64);
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
        Self { game: None, base }
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

    /// Index of the drum stem in `stem_paths()`, -1 for none.
    #[func]
    fn drum_stem(&self) -> i64 {
        self.level_field(|l| l.drum_stem).unwrap_or(-1)
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
        let mut d = VarDictionary::new();
        match read_wav_mono(&path.to_string()) {
            Ok((samples, sr)) => {
                let a = analyze(&samples, sr);
                d.set("bpm", a.bpm);
                d.set("offset_s", a.offset_s);
                d.set("tempo_confidence", a.tempo_confidence);
                d.set("phase_confidence", a.phase_confidence);
                d.set("beats", a.beats);
                d.set("bars", a.bars);
                d.set("duration_s", a.duration_s);
            }
            Err(e) => godot_error!("downbeat: analyze: {e}"),
        }
        d
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
        let mut d = VarDictionary::new();
        let level = match Level::from_json(&json.to_string()) {
            Ok(l) => l,
            Err(e) => {
                godot_error!("downbeat: level json: {e}");
                return d;
            }
        };
        match crate::chart::compile(&level, &crate::patterns::builtin_lib()) {
            Ok(chart) => {
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
                d.set("fingerprint", format!("{:016x}", chart.fingerprint()).as_str());
            }
            Err(e) => godot_error!("downbeat: compile: {e}"),
        }
        d
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
        if let Some(to) = t.to {
            d.set("to_yaw", to.yaw as f64);
            d.set("to_pitch", to.pitch as f64);
            d.set("to_dist", to.dist as f64);
        }
        d
    }

    /// Position of a target at a chart time, in the lane frame (metres,
    /// Y up, lane along -Z). Tracking targets move; everything else is fixed.
    #[func]
    fn target_position_at(&self, id: i64, chart_time: f64) -> Vector3 {
        match self.game.as_ref().and_then(|g| g.target(id.max(0) as u32)) {
            Some(t) => {
                let p = t.aim_at(chart_time).position();
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
        match Game::from_level(level, cfg, Windows::default(), Scoring::default()) {
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
