//! BPM-space charts.
//!
//! A `Pattern` is a phrase authored in beats: where targets appear (angles
//! relative to the lane origin), when they must be shot, how long they are
//! visible, and for tracking targets where they travel. A `Level` is a list of
//! patterns and a BPM. `compile` turns that into a `Chart`: absolute targets
//! with spawn, hit and end times in seconds.
//!
//! Nothing here knows about the renderer. The engine spawns a visual for each
//! `Target` at `spawn_time`, uses `hit_time` to drive its approach animation,
//! and the judge tells it when the target is consumed or expired.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// One target, one shot, on the beat. The core aim-trainer "click-timing" skill.
    Flick,
    /// Several targets visible at once; the chart dictates the order. Target switching.
    Switch,
    /// A moving target held under fire for `hold` beats. Scored per tick.
    Track,
    /// Small angular steps on each beat. Micro-adjustment.
    Micro,
    /// Sixteenth-note bursts: a drum fill. Speed under a short lead.
    Rush,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Flick => "FLICK",
            Kind::Switch => "SWITCH",
            Kind::Track => "TRACK",
            Kind::Micro => "MICRO",
            Kind::Rush => "RUSH",
        }
    }
}

/// Angular placement relative to the lane origin (the spot the player stands
/// on, facing the lane). Yaw positive = right, pitch positive = up, degrees.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Aim {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
}

impl Aim {
    pub const fn new(yaw: f32, pitch: f32, dist: f32) -> Self {
        Self { yaw, pitch, dist }
    }

    /// Unit direction in a right-handed, Y-up frame where the lane runs along -Z.
    pub fn direction(&self) -> [f32; 3] {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();
        let cp = pitch.cos();
        [yaw.sin() * cp, pitch.sin(), -yaw.cos() * cp]
    }

    pub fn position(&self) -> [f32; 3] {
        let d = self.direction();
        [d[0] * self.dist, d[1] * self.dist, d[2] * self.dist]
    }

    pub fn lerp(&self, other: &Aim, t: f32) -> Aim {
        Aim {
            yaw: self.yaw + (other.yaw - self.yaw) * t,
            pitch: self.pitch + (other.pitch - self.pitch) * t,
            dist: self.dist + (other.dist - self.dist) * t,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Hit beat relative to the pattern start.
    pub beat: f64,
    pub kind: Kind,
    pub aim: Aim,
    /// Target radius in metres.
    pub radius: f32,
    /// Beats the target is visible before its hit beat.
    pub lead: f64,
    /// Track only: beats the target must be held, starting at `beat`.
    #[serde(default)]
    pub hold: f64,
    /// Track only: where the target travels to by the end of `hold`.
    #[serde(default)]
    pub to: Option<Aim>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pattern {
    pub name: String,
    /// Phrase length in beats; the next pattern starts here.
    pub length: f64,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Section {
    pub pattern: String,
    pub repeats: u32,
}

/// How authored geometry scales with BPM. Exponent 0 disables scaling.
/// radius' = radius * (ref_bpm / bpm) ^ radius_exp
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Scaling {
    pub ref_bpm: f64,
    pub radius_exp: f64,
}

impl Default for Scaling {
    fn default() -> Self {
        Self { ref_bpm: 128.0, radius_exp: 0.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Level {
    pub name: String,
    pub bpm: f64,
    /// Silent beats before the first pattern. Gives the player the pulse first.
    #[serde(default = "default_lead_in")]
    pub lead_in: f64,
    pub sections: Vec<Section>,
    #[serde(default)]
    pub scaling: Scaling,
    /// Patterns defined inline with the level. Names here shadow the builtin library.
    #[serde(default)]
    pub patterns: Vec<Pattern>,
    /// Audio stream for the run (engine path). Empty = procedural click track.
    #[serde(default)]
    pub audio: String,
    /// Stems played in sync with `audio` and faded by combo tier (engine paths).
    #[serde(default)]
    pub stems: Vec<String>,
    /// Seconds into the audio file at which beat 0 falls.
    #[serde(default)]
    pub song_offset_s: f64,
    /// Index into `stems` of the drum stem: the player's part. It is audible
    /// through the lead-in and muted once targets start; the kit samples are
    /// what the player hears for every hit. -1 = none.
    #[serde(default = "default_drum_stem")]
    pub drum_stem: i64,
    /// Kit samples by target kind label (FLICK, SWITCH, MICRO, TRACK, RUSH,
    /// DEAD). Empty = the engine's synthesised kit.
    #[serde(default)]
    pub kit: std::collections::BTreeMap<String, String>,
}

fn default_drum_stem() -> i64 {
    -1
}

fn default_lead_in() -> f64 {
    8.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub id: u32,
    pub kind: Kind,
    pub aim: Aim,
    pub to: Option<Aim>,
    pub radius: f32,
    pub hit_beat: f64,
    pub spawn_beat: f64,
    /// Track: hit_beat + hold. Others: hit_beat.
    pub end_beat: f64,
    pub hit_time: f64,
    pub spawn_time: f64,
    pub end_time: f64,
    /// Which pattern instance produced this target (sequential), for stats.
    pub phrase: u32,
    pub pattern: String,
}

impl Target {
    /// Position of a Track target at song time `t` (clamped to its path).
    pub fn aim_at(&self, t: f64) -> Aim {
        match (self.kind, self.to) {
            (Kind::Track, Some(to)) if self.end_time > self.hit_time => {
                let u = ((t - self.hit_time) / (self.end_time - self.hit_time)).clamp(0.0, 1.0);
                self.aim.lerp(&to, u as f32)
            }
            _ => self.aim,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Chart {
    pub name: String,
    pub bpm: f64,
    pub total_beats: f64,
    pub total_time: f64,
    /// Sorted by hit_time.
    pub targets: Vec<Target>,
    /// Indices into `targets`, sorted by spawn_time.
    pub spawn_order: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompileError {
    UnknownPattern(String),
    BadBpm(f64),
    EmptyLevel,
    TrackWithoutHold(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnknownPattern(p) => write!(f, "unknown pattern '{p}'"),
            CompileError::BadBpm(b) => write!(f, "bpm {b} is not positive"),
            CompileError::EmptyLevel => write!(f, "level has no sections"),
            CompileError::TrackWithoutHold(p) => {
                write!(f, "pattern '{p}' has a track event with hold <= 0")
            }
        }
    }
}

impl std::error::Error for CompileError {}

pub type PatternLib = HashMap<String, Pattern>;

pub fn compile(level: &Level, lib: &PatternLib) -> Result<Chart, CompileError> {
    if level.bpm <= 0.0 {
        return Err(CompileError::BadBpm(level.bpm));
    }
    if level.sections.is_empty() {
        return Err(CompileError::EmptyLevel);
    }
    let spb = 60.0 / level.bpm;
    let radius_scale = (level.scaling.ref_bpm / level.bpm).powf(level.scaling.radius_exp) as f32;

    let inline: HashMap<&str, &Pattern> =
        level.patterns.iter().map(|p| (p.name.as_str(), p)).collect();
    let lookup = |name: &str| -> Option<&Pattern> {
        inline.get(name).copied().or_else(|| lib.get(name))
    };

    let mut targets = Vec::new();
    let mut cursor = level.lead_in;
    let mut phrase: u32 = 0;
    let mut id: u32 = 0;
    for section in &level.sections {
        let pattern = lookup(&section.pattern)
            .ok_or_else(|| CompileError::UnknownPattern(section.pattern.clone()))?;
        for _ in 0..section.repeats {
            for e in &pattern.events {
                if e.kind == Kind::Track && e.hold <= 0.0 {
                    return Err(CompileError::TrackWithoutHold(pattern.name.clone()));
                }
                let hit_beat = cursor + e.beat;
                let spawn_beat = hit_beat - e.lead;
                let end_beat = if e.kind == Kind::Track { hit_beat + e.hold } else { hit_beat };
                targets.push(Target {
                    id,
                    kind: e.kind,
                    aim: e.aim,
                    to: if e.kind == Kind::Track { e.to.or(Some(e.aim)) } else { None },
                    radius: e.radius * radius_scale,
                    hit_beat,
                    spawn_beat,
                    end_beat,
                    hit_time: hit_beat * spb,
                    spawn_time: spawn_beat * spb,
                    end_time: end_beat * spb,
                    phrase,
                    pattern: pattern.name.clone(),
                });
                id += 1;
            }
            cursor += pattern.length;
            phrase += 1;
        }
    }
    targets.sort_by(|a, b| a.hit_time.partial_cmp(&b.hit_time).unwrap());
    for (i, t) in targets.iter_mut().enumerate() {
        t.id = i as u32;
    }
    let mut spawn_order: Vec<usize> = (0..targets.len()).collect();
    spawn_order.sort_by(|&a, &b| {
        targets[a]
            .spawn_time
            .partial_cmp(&targets[b].spawn_time)
            .unwrap()
            .then(a.cmp(&b))
    });
    let total_beats = cursor + 4.0;
    Ok(Chart {
        name: level.name.clone(),
        bpm: level.bpm,
        total_beats,
        total_time: total_beats * spb,
        targets,
        spawn_order,
    })
}

impl Chart {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("chart serialises")
    }

    /// Stable 64-bit fingerprint of what the player is judged against: tempo
    /// and every target's beat, kind, placement and size. Replays carry it so
    /// a run can never be re-scored against a chart that has since changed.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |bytes: &[u8]| {
            for b in bytes {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        feed(&self.bpm.to_bits().to_le_bytes());
        for t in &self.targets {
            feed(&[t.kind as u8]);
            feed(&t.hit_beat.to_bits().to_le_bytes());
            feed(&t.spawn_beat.to_bits().to_le_bytes());
            feed(&t.end_beat.to_bits().to_le_bytes());
            feed(&t.radius.to_bits().to_le_bytes());
            for v in [t.aim.yaw, t.aim.pitch, t.aim.dist] {
                feed(&v.to_bits().to_le_bytes());
            }
            if let Some(to) = &t.to {
                for v in [to.yaw, to.pitch, to.dist] {
                    feed(&v.to_bits().to_le_bytes());
                }
            }
        }
        h
    }
}

impl Level {
    pub fn from_json(s: &str) -> Result<Level, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("level serialises")
    }
}
