//! Builtin pattern library and the warmup level.
//!
//! Every pattern is a 4-beat phrase unless noted. Angles are degrees from the
//! lane origin; distances in metres; radii in metres. Leads are in beats, so
//! at 174 BPM a 1-beat lead is 345 ms and at 90 BPM it is 667 ms. That is the
//! BPM difficulty curve working as intended: same chart, faster song, less time.

use crate::chart::{Aim, Event, Kind, Level, Pattern, PatternLib, Scaling, Section};

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
        scaling: Scaling::default(),
        patterns: vec![],
        audio: String::new(),
        stems: vec![],
        song_offset_s: 0.0,
        drum_stem: -1,
        kit: Default::default(),
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
        scaling: Scaling::default(),
        patterns: vec![],
        audio: String::new(),
        stems: vec![],
        song_offset_s: 0.0,
        drum_stem: -1,
        kit: Default::default(),
    }
}
