//! BPM-space charts for a linear shooter.
//!
//! A `Pattern` is a phrase authored in beats: which enemy appears where
//! (angles from the room's stand point), the beat its weak point must be shot,
//! how long it is exposed before that beat, and for striders the path it
//! crosses. A `Level` is rooms plus a sequence of segments: traversal (bars to
//! move) and encounters (a room and the patterns played in it). `compile`
//! turns that into a `Chart`: absolute targets with world positions and spawn,
//! hit and end times, plus the segment spans the engine opens doors on.
//!
//! Nothing here knows about the renderer. The engine spawns an enemy for each
//! `Target` at `spawn_time`, drives its approach from `hit_time`, and the
//! judge tells it when the weak point was hit or the enemy got its shot off.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aim-scenario kind of a target. Each kind is one enemy type.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Drone: pops to a point, exposes its core on the beat. Click timing.
    Flick,
    /// Gunner pair: two peek alternately from cover. Target switching.
    Switch,
    /// Strider: crosses the room on a path; beam held on its core. Tracking.
    Track,
    /// Swarm bot: small, stepped positions on each beat. Micro-adjust.
    Micro,
    /// Heavy plate: one of a sixteenth-note sequence before the charge. Speed.
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

    /// Enemy type that carries this kind of weak point.
    pub fn enemy(self) -> &'static str {
        match self {
            Kind::Flick => "DRONE",
            Kind::Switch => "GUNNER",
            Kind::Track => "STRIDER",
            Kind::Micro => "SWARM",
            Kind::Rush => "HEAVY",
        }
    }

    /// Damage the enemy deals when it survives its window and fires.
    pub fn damage(self) -> u32 {
        match self {
            Kind::Flick => 10,
            Kind::Switch => 12,
            Kind::Track => 20,
            Kind::Micro => 4,
            Kind::Rush => 6,
        }
    }
}

/// Angular placement relative to a room's stand point, facing the room's
/// forward axis. Yaw positive = right, pitch positive = up, degrees.
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

    /// Unit direction in a right-handed, Y-up frame with forward along -Z.
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
    /// Weak-point radius in metres.
    pub radius: f32,
    /// Beats the enemy is exposed before its hit beat.
    pub lead: f64,
    /// Track only: beats the strider must be held, starting at `beat`.
    #[serde(default)]
    pub hold: f64,
    /// Track only: where the strider travels to by the end of `hold`.
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

/// A greybox block inside a room (cover, perch, pillar), relative to the room origin.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub pos: [f32; 3],
    pub size: [f32; 3],
}

/// A room: a box the player fights in. `stand` is the point patterns are
/// authored from, relative to `origin`; `yaw_deg` turns the room (0 = enemies
/// appear toward -Z). `origin` may be left unset and is laid out by `compile`
/// along the level's path.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Room {
    pub name: String,
    #[serde(default)]
    pub origin: Option<[f32; 3]>,
    /// Width (x), height (y), depth (z) of the room box.
    pub size: [f32; 3],
    /// Player stand point relative to origin (y is floor height of the stand).
    pub stand: [f32; 3],
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

impl Room {
    /// World position of an aim relative to this room's stand point (eye
    /// height 1.6 m). Weak points are kept between knee height and the
    /// ceiling: an authored downward angle lands on a low enemy, never in the floor.
    pub fn world(&self, aim: &Aim) -> [f32; 3] {
        let o = self.origin.unwrap_or([0.0; 3]);
        let p = aim.position();
        let (s, c) = self.yaw_deg.to_radians().sin_cos();
        let x = p[0] * c + p[2] * s;
        let z = -p[0] * s + p[2] * c;
        let floor = o[1];
        let y = (o[1] + self.stand[1] + 1.6 + p[1]).clamp(floor + 0.6, floor + self.size[1] - 0.5);
        [o[0] + self.stand[0] + x, y, o[2] + self.stand[2] + z]
    }

    pub fn stand_world(&self) -> [f32; 3] {
        let o = self.origin.unwrap_or([0.0; 3]);
        [o[0] + self.stand[0], o[1] + self.stand[1], o[2] + self.stand[2]]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Segment {
    /// Bars to move from the previous room to the next. Doors open at its start.
    Traversal { bars: f64 },
    /// A fight in `room`: the patterns, in order.
    Encounter { room: String, sections: Vec<Section> },
    /// A boss in `room`: a chart within the chart. Each phase is a section
    /// list keyed to a part of the song and judged as a whole: when the phase
    /// ends, the kills it took are counted against `clear`; below it the boss
    /// lands its attack, the player is down and the phase restarts. Every
    /// phase is its own restart point.
    Boss { room: String, phases: Vec<Phase> },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Phase {
    #[serde(default)]
    pub name: String,
    pub sections: Vec<Section>,
    /// Fraction of the phase's weak points that must die for the phase to clear.
    #[serde(default = "default_clear")]
    pub clear: f64,
}

fn default_clear() -> f64 {
    0.7
}

/// A set piece on a bar: the engine plays it, the model only times it.
/// `kind` is the engine's cue name (train, lights, reveal, shake...).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Cue {
    /// Bar (1-based, as charted) on whose downbeat the cue starts.
    pub bar: f64,
    pub kind: String,
    #[serde(default)]
    pub room: String,
    /// Length in bars, for cues that run (a train, a light fade).
    #[serde(default)]
    pub bars: f64,
    #[serde(default)]
    pub param: f64,
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

/// Speed the level layout assumes for traversal, metres per second.
pub const TRAVEL_SPEED: f32 = 6.0;
/// Fraction of the traversal budget the corridor length consumes; the rest is slack.
pub const TRAVEL_FILL: f32 = 0.75;
pub const CORRIDOR_WIDTH: f32 = 6.0;
pub const CORRIDOR_HEIGHT: f32 = 5.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Level {
    pub name: String,
    /// Song title shown in menus; levels with the same `song` are one song
    /// pack with several difficulties.
    #[serde(default)]
    pub song: String,
    #[serde(default)]
    pub artist: String,
    /// "Normal", "Hard", "Insane"... Empty for Range drills.
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub charter: String,
    pub bpm: f64,
    /// Silent beats before the first segment. Gives the player the pulse first.
    #[serde(default = "default_lead_in")]
    pub lead_in: f64,
    /// Shorthand: a single encounter in the default range room.
    #[serde(default)]
    pub sections: Vec<Section>,
    /// The level's path. Empty = `sections` in the range room.
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// Rooms referenced by encounters. Empty = the default range room.
    #[serde(default)]
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub scaling: Scaling,
    /// Patterns defined inline with the level. Names here shadow the builtin library.
    #[serde(default)]
    pub patterns: Vec<Pattern>,
    /// Audio stream for the run (engine path). Empty = procedural click track.
    #[serde(default)]
    pub audio: String,
    /// Stems played in sync with `audio` and opened by combo tier (engine paths).
    #[serde(default)]
    pub stems: Vec<String>,
    /// Seconds into the audio file at which beat 0 falls.
    #[serde(default)]
    pub song_offset_s: f64,
    /// Hit-confirm samples by kind label (FLICK, SWITCH, MICRO, TRACK, RUSH,
    /// DEAD). Empty = the engine's synthesised kit.
    #[serde(default)]
    pub kit: std::collections::BTreeMap<String, String>,
    /// Set pieces on bars.
    #[serde(default)]
    pub cues: Vec<Cue>,
}

fn default_lead_in() -> f64 {
    8.0
}

impl Default for Level {
    fn default() -> Self {
        Self {
            name: String::new(),
            song: String::new(),
            artist: String::new(),
            difficulty: String::new(),
            charter: String::new(),
            bpm: 128.0,
            lead_in: 8.0,
            sections: vec![],
            segments: vec![],
            rooms: vec![],
            scaling: Scaling::default(),
            patterns: vec![],
            audio: String::new(),
            stems: vec![],
            song_offset_s: 0.0,
            cues: vec![],
            kit: Default::default(),
        }
    }
}

/// The lane the Range drills play in: 48 m wide, 60 m deep, stand 14 m in.
pub fn range_room() -> Room {
    Room {
        name: "range".into(),
        origin: Some([0.0, 0.0, 0.0]),
        size: [48.0, 14.0, 60.0],
        stand: [0.0, 0.0, 14.0],
        yaw_deg: 0.0,
        blocks: vec![
            Block { pos: [-9.0, 2.5, -8.0], size: [2.0, 5.0, 2.0] },
            Block { pos: [9.0, 2.5, -8.0], size: [2.0, 5.0, 2.0] },
        ],
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub id: u32,
    pub kind: Kind,
    /// Aim relative to the room stand (what the rating reads).
    pub aim: Aim,
    pub to: Option<Aim>,
    /// Weak-point world position at spawn, and at path end for striders.
    pub pos: [f32; 3],
    pub to_pos: Option<[f32; 3]>,
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
    pub room: String,
    /// Index of the encounter segment this target belongs to.
    pub segment: u32,
    /// Boss phase (0-based within the boss) when the target is a boss weak point.
    #[serde(default)]
    pub phase: Option<u32>,
}

impl Target {
    /// Aim of a strider at chart time `t` (clamped to its path).
    pub fn aim_at(&self, t: f64) -> Aim {
        match (self.kind, self.to) {
            (Kind::Track, Some(to)) if self.end_time > self.hit_time => {
                let u = ((t - self.hit_time) / (self.end_time - self.hit_time)).clamp(0.0, 1.0);
                self.aim.lerp(&to, u as f32)
            }
            _ => self.aim,
        }
    }

    /// World position at chart time `t`.
    pub fn pos_at(&self, t: f64) -> [f32; 3] {
        match (self.kind, self.to_pos) {
            (Kind::Track, Some(to)) if self.end_time > self.hit_time => {
                let u = ((t - self.hit_time) / (self.end_time - self.hit_time)).clamp(0.0, 1.0) as f32;
                [
                    self.pos[0] + (to[0] - self.pos[0]) * u,
                    self.pos[1] + (to[1] - self.pos[1]) * u,
                    self.pos[2] + (to[2] - self.pos[2]) * u,
                ]
            }
            _ => self.pos,
        }
    }
}

/// A segment with its resolved beat span, for doors, traversal budgets and the HUD.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub index: u32,
    pub kind: String,
    pub room: String,
    pub start_beat: f64,
    pub end_beat: f64,
    pub start_time: f64,
    pub end_time: f64,
    /// Boss spans: the phase number and name.
    #[serde(default)]
    pub phase: Option<u32>,
    #[serde(default)]
    pub label: String,
}

/// A boss phase as compiled: the judge closes it at `end_time` plus the late
/// window and counts kills against `clear`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PhaseInfo {
    pub segment: u32,
    pub phase: u32,
    pub name: String,
    pub room: String,
    pub start_beat: f64,
    pub end_beat: f64,
    pub start_time: f64,
    pub end_time: f64,
    pub targets: u32,
    pub clear: f64,
    /// Last phase of its boss: clearing it kills the boss.
    pub last: bool,
}

/// A set piece placed in time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CueInfo {
    pub kind: String,
    pub room: String,
    pub beat: f64,
    pub time: f64,
    pub bars: f64,
    pub param: f64,
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
    /// Rooms with origins resolved, in path order of first use.
    pub rooms: Vec<Room>,
    pub spans: Vec<Span>,
    #[serde(default)]
    pub phases: Vec<PhaseInfo>,
    #[serde(default)]
    pub cues: Vec<CueInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompileError {
    UnknownPattern(String),
    UnknownRoom(String),
    BadBpm(f64),
    EmptyLevel,
    TrackWithoutHold(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnknownPattern(p) => write!(f, "unknown pattern '{p}'"),
            CompileError::UnknownRoom(r) => write!(f, "unknown room '{r}'"),
            CompileError::BadBpm(b) => write!(f, "bpm {b} is not positive"),
            CompileError::EmptyLevel => write!(f, "level has no segments or sections"),
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
    let segments: Vec<Segment> = if !level.segments.is_empty() {
        level.segments.clone()
    } else if !level.sections.is_empty() {
        vec![Segment::Encounter { room: "range".into(), sections: level.sections.clone() }]
    } else {
        return Err(CompileError::EmptyLevel);
    };
    let spb = 60.0 / level.bpm;
    let radius_scale = (level.scaling.ref_bpm / level.bpm).powf(level.scaling.radius_exp) as f32;

    let inline: HashMap<&str, &Pattern> =
        level.patterns.iter().map(|p| (p.name.as_str(), p)).collect();
    let lookup = |name: &str| -> Option<&Pattern> {
        inline.get(name).copied().or_else(|| lib.get(name))
    };
    let mut room_defs: HashMap<String, Room> =
        level.rooms.iter().map(|r| (r.name.clone(), r.clone())).collect();
    room_defs.entry("range".into()).or_insert_with(range_room);

    // Lay rooms out along -Z in path order; a traversal's corridor length is
    // what the player can cover in its bar budget. Authored origins win.
    let mut rooms: Vec<Room> = Vec::new();
    let mut room_index: HashMap<String, usize> = HashMap::new();
    let mut cursor_z: f32 = 0.0;
    let mut pending_corridor: f32 = 0.0;
    let mut spans = Vec::new();
    let mut targets = Vec::new();
    let mut phases = Vec::new();
    let mut cursor = level.lead_in;
    let mut phrase: u32 = 0;
    let mut id: u32 = 0;
    // Segment ids: one per traversal or encounter, one per boss phase, so a
    // boss phase checkpoints and restarts on its own.
    let mut seg_id: u32 = 0;
    for seg in segments.iter() {
        match seg {
            Segment::Traversal { bars } => {
                let seconds = bars * 4.0 * spb;
                pending_corridor += (seconds as f32) * TRAVEL_SPEED * TRAVEL_FILL;
                spans.push(Span {
                    index: seg_id,
                    kind: "traversal".into(),
                    room: String::new(),
                    start_beat: cursor,
                    end_beat: cursor + bars * 4.0,
                    start_time: cursor * spb,
                    end_time: (cursor + bars * 4.0) * spb,
                    phase: None,
                    label: String::new(),
                });
                cursor += bars * 4.0;
                seg_id += 1;
            }
            Segment::Encounter { room, sections } => {
                let placed = place_room(room, &room_defs, &mut rooms, &mut room_index, &mut cursor_z, &mut pending_corridor)?;
                let start = cursor;
                emit_sections(sections, &lookup, &placed, room, seg_id, None, radius_scale, spb, &mut cursor, &mut phrase, &mut id, &mut targets)?;
                spans.push(Span {
                    index: seg_id,
                    kind: "encounter".into(),
                    room: room.clone(),
                    start_beat: start,
                    end_beat: cursor,
                    start_time: start * spb,
                    end_time: cursor * spb,
                    phase: None,
                    label: String::new(),
                });
                seg_id += 1;
            }
            Segment::Boss { room, phases: boss_phases } => {
                let placed = place_room(room, &room_defs, &mut rooms, &mut room_index, &mut cursor_z, &mut pending_corridor)?;
                for (k, ph) in boss_phases.iter().enumerate() {
                    let start = cursor;
                    let before = targets.len();
                    emit_sections(&ph.sections, &lookup, &placed, room, seg_id, Some(k as u32), radius_scale, spb, &mut cursor, &mut phrase, &mut id, &mut targets)?;
                    let name = if ph.name.is_empty() { format!("phase {}", k + 1) } else { ph.name.clone() };
                    spans.push(Span {
                        index: seg_id,
                        kind: "boss".into(),
                        room: room.clone(),
                        start_beat: start,
                        end_beat: cursor,
                        start_time: start * spb,
                        end_time: cursor * spb,
                        phase: Some(k as u32),
                        label: name.clone(),
                    });
                    phases.push(PhaseInfo {
                        segment: seg_id,
                        phase: k as u32,
                        name,
                        room: room.clone(),
                        start_beat: start,
                        end_beat: cursor,
                        start_time: start * spb,
                        end_time: cursor * spb,
                        targets: (targets.len() - before) as u32,
                        clear: ph.clear.clamp(0.0, 1.0),
                        last: k + 1 == boss_phases.len(),
                    });
                    seg_id += 1;
                }
            }
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
    let cues: Vec<CueInfo> = level
        .cues
        .iter()
        .map(|c| {
            let beat = (c.bar - 1.0) * 4.0;
            CueInfo { kind: c.kind.clone(), room: c.room.clone(), beat, time: beat * spb, bars: c.bars, param: c.param }
        })
        .collect();
    Ok(Chart {
        name: level.name.clone(),
        bpm: level.bpm,
        total_beats,
        total_time: total_beats * spb,
        targets,
        spawn_order,
        rooms,
        spans,
        phases,
        cues,
    })
}

/// Place a room on the path (first use lays it out past the pending corridor).
fn place_room(
    room: &str,
    room_defs: &HashMap<String, Room>,
    rooms: &mut Vec<Room>,
    room_index: &mut HashMap<String, usize>,
    cursor_z: &mut f32,
    pending_corridor: &mut f32,
) -> Result<Room, CompileError> {
    let def = room_defs.get(room).ok_or_else(|| CompileError::UnknownRoom(room.to_string()))?;
    let room_i = match room_index.get(room) {
        Some(i) => *i,
        None => {
            let mut r = def.clone();
            if r.origin.is_none() {
                // Room box is centred on origin in x/z; the entrance is its +Z
                // face. Place it past the pending corridor.
                *cursor_z -= *pending_corridor + r.size[2] * 0.5;
                r.origin = Some([0.0, 0.0, *cursor_z]);
                *cursor_z -= r.size[2] * 0.5;
                *pending_corridor = 0.0;
            }
            rooms.push(r);
            room_index.insert(room.to_string(), rooms.len() - 1);
            rooms.len() - 1
        }
    };
    Ok(rooms[room_i].clone())
}

/// Lay a section list's targets from `cursor` on, in `placed`.
#[allow(clippy::too_many_arguments)]
fn emit_sections<'a>(
    sections: &[Section],
    lookup: &dyn Fn(&str) -> Option<&'a Pattern>,
    placed: &Room,
    room: &str,
    seg_id: u32,
    phase: Option<u32>,
    radius_scale: f32,
    spb: f64,
    cursor: &mut f64,
    phrase: &mut u32,
    id: &mut u32,
    targets: &mut Vec<Target>,
) -> Result<(), CompileError> {
    for section in sections {
        let pattern = lookup(&section.pattern).ok_or_else(|| CompileError::UnknownPattern(section.pattern.clone()))?;
        for _ in 0..section.repeats {
            for e in &pattern.events {
                if e.kind == Kind::Track && e.hold <= 0.0 {
                    return Err(CompileError::TrackWithoutHold(pattern.name.clone()));
                }
                let hit_beat = *cursor + e.beat;
                let spawn_beat = hit_beat - e.lead;
                let end_beat = if e.kind == Kind::Track { hit_beat + e.hold } else { hit_beat };
                let to = if e.kind == Kind::Track { e.to.or(Some(e.aim)) } else { None };
                targets.push(Target {
                    id: *id,
                    kind: e.kind,
                    aim: e.aim,
                    to,
                    pos: placed.world(&e.aim),
                    to_pos: to.map(|a| placed.world(&a)),
                    radius: e.radius * radius_scale,
                    hit_beat,
                    spawn_beat,
                    end_beat,
                    hit_time: hit_beat * spb,
                    spawn_time: spawn_beat * spb,
                    end_time: end_beat * spb,
                    phrase: *phrase,
                    pattern: pattern.name.clone(),
                    room: room.to_string(),
                    segment: seg_id,
                    phase,
                });
                *id += 1;
            }
            *cursor += pattern.length;
            *phrase += 1;
        }
    }
    Ok(())
}

impl Chart {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("chart serialises")
    }

    pub fn room(&self, name: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.name == name)
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
            for v in t.pos {
                feed(&v.to_bits().to_le_bytes());
            }
            if let Some(to) = &t.to_pos {
                for v in to {
                    feed(&v.to_bits().to_le_bytes());
                }
            }
        }
        for p in &self.phases {
            feed(&p.end_time.to_bits().to_le_bytes());
            feed(&p.clear.to_bits().to_le_bytes());
            feed(&p.targets.to_le_bytes());
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
