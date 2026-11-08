//! Builtin pattern library and the warmup level.
//!
//! Every pattern is a 4-beat phrase unless noted. Angles are degrees from the
//! lane origin; distances in metres; radii in metres. Leads are in beats, so
//! at 174 BPM a 1-beat lead is 345 ms and at 90 BPM it is 667 ms. That is the
//! BPM difficulty curve working as intended: same chart, faster song, less time.

use crate::chart::{Aim, Block, Event, Kind, Level, Pattern, PatternLib, Room, Section, Segment};

const LANE_DIST: f32 = 12.0;

fn flick(beat: f64, yaw: f32, pitch: f32, radius: f32, lead: f64) -> Event {
    Event {
        beat,
        kind: Kind::Flick,
        aim: Aim::new(yaw, pitch, LANE_DIST),
        radius,
        lead,
        hold: 0.0,
        to: None,
    }
}

fn switch(beat: f64, yaw: f32, pitch: f32, radius: f32, lead: f64) -> Event {
    Event { kind: Kind::Switch, ..flick(beat, yaw, pitch, radius, lead) }
}

fn micro(beat: f64, yaw: f32, pitch: f32, radius: f32, lead: f64) -> Event {
    Event { kind: Kind::Micro, ..flick(beat, yaw, pitch, radius, lead) }
}

fn rush(beat: f64, yaw: f32, pitch: f32, radius: f32, lead: f64) -> Event {
    Event { kind: Kind::Rush, ..flick(beat, yaw, pitch, radius, lead) }
}

fn track(beat: f64, from: (f32, f32), to: (f32, f32), hold: f64, radius: f32, lead: f64) -> Event {
    Event {
        beat,
        kind: Kind::Track,
        aim: Aim::new(from.0, from.1, LANE_DIST),
        radius,
        lead,
        hold,
        to: Some(Aim::new(to.0, to.1, LANE_DIST)),
    }
}

pub fn builtin_patterns() -> Vec<Pattern> {
    vec![
        Pattern {
            name: "flick_lr".into(),
            length: 4.0,
            events: vec![
                flick(0.0, -22.0, 0.0, 0.45, 1.0),
                flick(1.0, 22.0, 0.0, 0.45, 1.0),
                flick(2.0, -22.0, 0.0, 0.45, 1.0),
                flick(3.0, 22.0, 0.0, 0.45, 1.0),
            ],
        },
        Pattern {
            name: "flick_quad".into(),
            length: 4.0,
            events: vec![
                flick(0.0, -18.0, 8.0, 0.45, 1.0),
                flick(1.0, 18.0, 8.0, 0.45, 1.0),
                flick(2.0, 18.0, -6.0, 0.45, 1.0),
                flick(3.0, -18.0, -6.0, 0.45, 1.0),
            ],
        },
        Pattern {
            name: "flick_8ths".into(),
            length: 4.0,
            events: vec![
                flick(0.0, -14.0, 4.0, 0.5, 0.75),
                flick(0.5, 12.0, -3.0, 0.5, 0.75),
                flick(1.0, -8.0, 9.0, 0.5, 0.75),
                flick(1.5, 16.0, 2.0, 0.5, 0.75),
                flick(2.0, -20.0, -4.0, 0.5, 0.75),
                flick(2.5, 6.0, 7.0, 0.5, 0.75),
                flick(3.0, -10.0, -8.0, 0.5, 0.75),
                flick(3.5, 18.0, -2.0, 0.5, 0.75),
            ],
        },
        Pattern {
            name: "switch_pair".into(),
            length: 4.0,
            events: (0..8)
                .map(|i| {
                    let yaw = if i % 2 == 0 { -15.0 } else { 15.0 };
                    switch(i as f64 * 0.5, yaw, 0.0, 0.5, 1.0)
                })
                .collect(),
        },
        Pattern {
            name: "micro_walk".into(),
            length: 4.0,
            events: vec![
                micro(0.0, -6.0, 0.0, 0.3, 1.0),
                micro(1.0, -2.0, 0.0, 0.3, 1.0),
                micro(2.0, 2.0, 0.0, 0.3, 1.0),
                micro(3.0, 6.0, 0.0, 0.3, 1.0),
            ],
        },
        Pattern {
            name: "micro_zigzag".into(),
            length: 4.0,
            events: vec![
                micro(0.0, 4.0, 3.0, 0.3, 1.0),
                micro(1.0, 0.0, -3.0, 0.3, 1.0),
                micro(2.0, -4.0, 3.0, 0.3, 1.0),
                micro(3.0, 0.0, -3.0, 0.3, 1.0),
            ],
        },
        Pattern {
            name: "track_sweep".into(),
            length: 4.0,
            events: vec![track(0.0, (-20.0, 0.0), (20.0, 0.0), 4.0, 0.6, 1.0)],
        },
        Pattern {
            name: "track_arc".into(),
            length: 4.0,
            events: vec![track(0.0, (15.0, -5.0), (-15.0, 8.0), 4.0, 0.6, 1.0)],
        },
        // Two beats of quarter kicks, then a two-beat sixteenth fill sweeping
        // left to right and climbing: the drummer's run into the drop.
        Pattern {
            name: "rush_fill".into(),
            length: 4.0,
            events: {
                let mut v = vec![flick(0.0, -10.0, 0.0, 0.5, 1.0), flick(1.0, 10.0, 0.0, 0.5, 1.0)];
                for i in 0..8 {
                    let t = i as f32 / 7.0;
                    v.push(rush(2.0 + i as f64 * 0.25, -16.0 + 32.0 * t, -4.0 + 10.0 * t, 0.55, 0.5));
                }
                v
            },
        },
        Pattern { name: "rest".into(), length: 4.0, events: vec![] },
    ]
}

pub fn builtin_lib() -> PatternLib {
    builtin_patterns().into_iter().map(|p| (p.name.clone(), p)).collect()
}

fn sec(pattern: &str, repeats: u32) -> Section {
    Section { pattern: pattern.into(), repeats }
}

/// The reference level: one of each skill, in a musical order, 64 bars total
/// at any BPM you hand it.
pub fn warmup_level(bpm: f64) -> Level {
    Level {
        name: format!("warmup_{}", bpm.round() as i64),
        bpm,
        lead_in: 8.0,
        sections: vec![
            sec("flick_lr", 2),
            sec("flick_quad", 2),
            sec("switch_pair", 2),
            sec("rest", 1),
            sec("micro_walk", 1),
            sec("micro_zigzag", 1),
            sec("track_sweep", 1),
            sec("track_arc", 1),
            sec("rest", 1),
            sec("flick_8ths", 2),
            sec("switch_pair", 1),
            sec("flick_lr", 1),
        ],
        ..Level::default()
    }
}

/// The second builtin: every kind including RUSH, denser than the warmup.
pub fn fill_level(bpm: f64) -> Level {
    Level {
        name: format!("fill_{}", bpm.round() as i64),
        bpm,
        lead_in: 8.0,
        sections: vec![
            sec("flick_quad", 2),
            sec("switch_pair", 1),
            sec("rush_fill", 1),
            sec("micro_zigzag", 2),
            sec("track_sweep", 1),
            sec("rush_fill", 1),
            sec("flick_8ths", 2),
            sec("switch_pair", 1),
            sec("rush_fill", 1),
            sec("rest", 1),
        ],
        ..Level::default()
    }
}

fn block(x: f32, y: f32, z: f32, w: f32, h: f32, d: f32) -> Block {
    Block { pos: [x, y, z], size: [w, h, d] }
}

/// Act-one greybox rooms. Origins are laid out by the compiler along the path.
pub fn builtin_rooms() -> Vec<Room> {
    vec![
        // A street: wide, low cover both sides, a van in the middle distance.
        Room {
            name: "street".into(),
            origin: None,
            size: [30.0, 9.0, 40.0],
            stand: [0.0, 0.0, 14.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(-6.0, 0.6, 2.0, 3.0, 1.2, 1.5),
                block(6.0, 0.6, 2.0, 3.0, 1.2, 1.5),
                block(-10.0, 1.0, -6.0, 2.0, 2.0, 5.0),
                block(10.0, 1.0, -6.0, 2.0, 2.0, 5.0),
                block(0.0, 1.3, -12.0, 5.0, 2.6, 2.4),
            ],
        },
        // An atrium: tall, perches high left and right, a central pillar.
        Room {
            name: "atrium".into(),
            origin: None,
            size: [26.0, 16.0, 30.0],
            stand: [0.0, 0.0, 10.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(-9.0, 4.0, -8.0, 4.0, 0.4, 4.0),
                block(9.0, 4.0, -8.0, 4.0, 0.4, 4.0),
                block(0.0, 3.0, -4.0, 1.6, 6.0, 1.6),
                block(-11.0, 2.0, -12.0, 3.0, 4.0, 3.0),
                block(11.0, 2.0, -12.0, 3.0, 4.0, 3.0),
            ],
        },
        // A metro platform: long and narrow, striders cross the tracks.
        Room {
            name: "platform".into(),
            origin: None,
            size: [36.0, 7.0, 34.0],
            stand: [0.0, 0.0, 12.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(0.0, -0.4, -8.0, 36.0, 0.8, 6.0),
                block(-14.0, 1.2, 2.0, 2.0, 2.4, 2.0),
                block(14.0, 1.2, 2.0, 2.0, 2.4, 2.0),
                block(0.0, 2.5, -14.0, 1.0, 5.0, 1.0),
            ],
        },
        // A flooded underpass: low, long, pillars in rows, a wrecked car.
        // The water is the engine's (cue "flood" raises it).
        Room {
            name: "underpass".into(),
            origin: None,
            size: [20.0, 5.0, 44.0],
            stand: [0.0, 0.0, 16.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(-6.0, 2.5, 4.0, 1.2, 5.0, 1.2),
                block(6.0, 2.5, 4.0, 1.2, 5.0, 1.2),
                block(-6.0, 2.5, -6.0, 1.2, 5.0, 1.2),
                block(6.0, 2.5, -6.0, 1.2, 5.0, 1.2),
                block(-6.0, 2.5, -16.0, 1.2, 5.0, 1.2),
                block(6.0, 2.5, -16.0, 1.2, 5.0, 1.2),
                block(-4.5, 0.65, -10.0, 3.8, 1.3, 1.8),
            ],
        },
        // A corporate lobby: a desk, two mezzanines, columns.
        Room {
            name: "lobby".into(),
            origin: None,
            size: [28.0, 12.0, 30.0],
            stand: [0.0, 0.0, 11.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(0.0, 0.55, -2.0, 8.0, 1.1, 1.5),
                block(-9.0, 4.5, -9.0, 10.0, 0.4, 6.0),
                block(9.0, 4.5, -9.0, 10.0, 0.4, 6.0),
                block(-5.0, 6.0, -12.0, 1.4, 12.0, 1.4),
                block(5.0, 6.0, -12.0, 1.4, 12.0, 1.4),
                block(-11.0, 0.5, 2.0, 2.0, 1.0, 2.0),
                block(11.0, 0.5, 2.0, 2.0, 1.0, 2.0),
            ],
        },
        // A rooftop: HVAC units, vent stacks, a raised deck, the parapet.
        Room {
            name: "rooftop".into(),
            origin: None,
            size: [34.0, 8.0, 36.0],
            stand: [0.0, 0.0, 13.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(-8.0, 1.1, -2.0, 4.0, 2.2, 3.0),
                block(8.0, 1.1, -2.0, 4.0, 2.2, 3.0),
                block(-12.0, 1.75, -10.0, 1.5, 3.5, 1.5),
                block(12.0, 1.75, -10.0, 1.5, 3.5, 1.5),
                block(0.0, 0.5, -12.0, 10.0, 1.0, 8.0),
                block(0.0, 0.6, -17.5, 34.0, 1.2, 0.8),
            ],
        },
        // A helipad: open, pad lights, floodlight masts, the crane base the
        // boss hangs from at the far end (the engine draws the boss).
        Room {
            name: "helipad".into(),
            origin: None,
            size: [40.0, 12.0, 40.0],
            stand: [0.0, 0.0, 14.0],
            yaw_deg: 0.0,
            blocks: vec![
                block(-8.0, 0.25, -6.0, 1.0, 0.5, 1.0),
                block(8.0, 0.25, -6.0, 1.0, 0.5, 1.0),
                block(-8.0, 0.25, -16.0, 1.0, 0.5, 1.0),
                block(8.0, 0.25, -16.0, 1.0, 0.5, 1.0),
                block(0.0, 5.0, -19.0, 3.0, 10.0, 3.0),
                block(-17.0, 4.5, -17.0, 0.6, 9.0, 0.6),
                block(17.0, 4.5, -17.0, 0.6, 9.0, 0.6),
            ],
        },
    ]
}

fn travel(bars: f64) -> Segment {
    Segment::Traversal { bars }
}

fn fight(room: &str, sections: Vec<Section>) -> Segment {
    Segment::Encounter { room: room.into(), sections }
}

/// Street, Hard: same rooms and song structure, denser patterns and fills.
pub fn street_hard_level(bpm: f64) -> Level {
    Level {
        name: format!("street_hard_{}", bpm.round() as i64),
        song: "Street".into(),
        artist: "click kit".into(),
        difficulty: "Hard".into(),
        charter: "builtin".into(),
        bpm,
        lead_in: 8.0,
        rooms: builtin_rooms(),
        segments: vec![
            travel(1.0),
            fight("street", vec![sec("flick_8ths", 2), sec("switch_pair", 2), sec("flick_quad", 1), sec("rush_fill", 1), sec("flick_8ths", 2)]),
            travel(2.0),
            fight("atrium", vec![sec("micro_zigzag", 2), sec("switch_pair", 2), sec("flick_8ths", 2), sec("rush_fill", 1), sec("micro_walk", 1)]),
            travel(2.0),
            fight("platform", vec![sec("track_arc", 1), sec("switch_pair", 2), sec("track_sweep", 1), sec("flick_8ths", 1), sec("rush_fill", 2), sec("flick_8ths", 1)]),
            travel(1.0),
        ],
        ..Level::default()
    }
}

/// Act one, level one: street -> atrium -> platform. Sixteen bars of fighting
/// per room, two bars to move between.
pub fn street_level(bpm: f64) -> Level {
    Level {
        name: format!("street_{}", bpm.round() as i64),
        song: "Street".into(),
        artist: "click kit".into(),
        difficulty: "Normal".into(),
        charter: "builtin".into(),
        bpm,
        lead_in: 8.0,
        rooms: builtin_rooms(),
        segments: vec![
            travel(1.0),
            fight("street", vec![sec("flick_lr", 2), sec("flick_quad", 2), sec("switch_pair", 2), sec("rest", 1), sec("flick_8ths", 1)]),
            travel(2.0),
            fight("atrium", vec![sec("micro_walk", 1), sec("micro_zigzag", 1), sec("flick_quad", 2), sec("switch_pair", 2), sec("flick_8ths", 2)]),
            travel(2.0),
            fight("platform", vec![sec("track_sweep", 1), sec("switch_pair", 1), sec("track_arc", 1), sec("flick_8ths", 1), sec("rush_fill", 1), sec("flick_quad", 1), sec("rush_fill", 1), sec("flick_lr", 1)]),
            travel(1.0),
        ],
        ..Level::default()
    }
}


/// A Range drill: one aim scenario as a phrase set, repeated to fill a duration.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Drill {
    pub name: String,
    /// Aim Lab-style skill label: click, switch, track, micro, speed, mixed.
    pub skill: String,
    pub description: String,
    /// One pass; `drill_level` repeats it.
    pub sections: Vec<Section>,
}

fn drill(name: &str, skill: &str, description: &str, sections: Vec<Section>) -> Drill {
    Drill { name: name.into(), skill: skill.into(), description: description.into(), sections }
}

pub fn drills() -> Vec<Drill> {
    vec![
        drill("gridshot", "click", "Quarter-note flicks across the grid.", vec![sec("flick_lr", 1), sec("flick_quad", 1)]),
        drill("eighths", "click", "Eighth-note flicks; the speed ceiling of click timing.", vec![sec("flick_8ths", 2)]),
        drill("switch", "switch", "Two gunners, alternating on eighths.", vec![sec("switch_pair", 2)]),
        drill("micro", "micro", "Small cores, small steps.", vec![sec("micro_walk", 1), sec("micro_zigzag", 1)]),
        drill("strafe", "track", "Striders crossing; hold the beam.", vec![sec("track_sweep", 1), sec("track_arc", 1)]),
        drill("fill", "speed", "Sixteenth plates before the drop.", vec![sec("flick_quad", 1), sec("rush_fill", 1)]),
        drill("warmup", "mixed", "Everything once, in a musical order.", warmup_level(128.0).sections),
    ]
}

pub fn drill_by_name(name: &str) -> Option<Drill> {
    drills().into_iter().find(|d| d.name == name)
}

/// Repeat a drill's pass to fill about `minutes` at `bpm`, in the range room.
pub fn drill_level(name: &str, bpm: f64, minutes: f64) -> Option<Level> {
    let d = drill_by_name(name)?;
    let lib = builtin_lib();
    let pass_beats: f64 = d
        .sections
        .iter()
        .map(|s| lib.get(&s.pattern).map(|p| p.length * s.repeats as f64).unwrap_or(4.0))
        .sum();
    let wanted_beats = (minutes.max(0.25) * 60.0 * bpm / 60.0) - 8.0;
    let passes = ((wanted_beats / pass_beats.max(1.0)).round() as usize).max(1);
    let mut sections = Vec::new();
    for _ in 0..passes {
        sections.extend(d.sections.iter().cloned());
    }
    Some(Level {
        name: format!("range_{}_{}", d.name, bpm.round() as i64),
        bpm,
        lead_in: 8.0,
        sections,
        ..Level::default()
    })
}
