//! Fair play: deterministic checks a leaderboard runs on every submitted
//! replay after re-scoring it. A flag is a reason to hold a score for review,
//! never a verdict on its own; the thresholds are mine and tune against
//! real players' replays.

use crate::replay::{Input, Replay};
use crate::score::Report;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Integrity {
    pub flags: Vec<String>,
    /// 0 = nothing of note; grows with each flag's weight.
    pub suspicion: f64,
    pub hits_checked: u32,
}

impl Integrity {
    pub fn clean(&self) -> bool {
        self.flags.is_empty()
    }
}

/// Minimum hits before timing statistics mean anything.
pub const MIN_HITS: u32 = 30;
/// Human timing never settles below this spread, even for top players.
pub const MIN_HUMAN_SD_MS: f64 = 3.0;
/// Mean precision above this over many hits means the ray is not a hand.
pub const MAX_HUMAN_PRECISION: f64 = 0.995;

pub fn inspect(report: &Report, replay: &Replay) -> Integrity {
    let mut flags = Vec::new();
    let mut suspicion = 0.0;
    let hits = report.hits;
    if hits >= MIN_HITS {
        if report.timing_std_ms < MIN_HUMAN_SD_MS {
            flags.push(format!("timing spread {:.2} ms over {} hits", report.timing_std_ms, hits));
            suspicion += 1.0;
        }
        if report.precision > MAX_HUMAN_PRECISION {
            flags.push(format!("precision {:.4} over {} hits", report.precision, hits));
            suspicion += 1.0;
        }
        // Quantised offsets: a macro fires on a grid; a hand does not.
        let mut quantised = 0usize;
        for h in &report.history {
            let frac = (h.delta_ms.abs() * 10.0).fract();
            if frac < 1e-6 || frac > 1.0 - 1e-6 {
                quantised += 1;
            }
        }
        if quantised as f64 > report.history.len() as f64 * 0.9 && !report.history.is_empty() {
            flags.push(format!("{} of {} offsets on a 0.1 ms grid", quantised, report.history.len()));
            suspicion += 0.5;
        }
    }
    // Replay structure: time must not run backwards except through a recorded restart.
    let mut last_at = f64::NEG_INFINITY;
    let mut back = 0;
    for input in &replay.inputs {
        let at = match input {
            Input::Shot { at, .. } | Input::Track { at, .. } | Input::Advance { at } => *at,
            Input::Mark { at, .. } | Input::Release { at, .. } => *at,
        };
        if at < last_at - 1e-9 && !matches!(input, Input::Advance { .. }) {
            back += 1;
        }
        if let Input::Advance { .. } = input {
            last_at = f64::NEG_INFINITY;
        } else {
            last_at = at;
        }
    }
    if back > 0 {
        flags.push(format!("{back} inputs out of time order"));
        suspicion += 2.0;
    }
    // Inhuman cadence: consecutive shots spaced by an exact multiple of a sixteenth.
    let spb = 60.0 / replay.level.bpm;
    let shots: Vec<f64> = replay
        .inputs
        .iter()
        .filter_map(|i| match i {
            Input::Shot { raw_time, .. } => Some(*raw_time),
            _ => None,
        })
        .collect();
    if shots.len() >= MIN_HITS as usize {
        let mut exact = 0usize;
        for w in shots.windows(2) {
            let gap = w[1] - w[0];
            let sixteenths = gap / (spb / 4.0);
            if (sixteenths - sixteenths.round()).abs() * (spb / 4.0) * 1000.0 < 0.05 {
                exact += 1;
            }
        }
        if exact as f64 > (shots.len() - 1) as f64 * 0.8 {
            flags.push(format!("{} of {} shot gaps are exact beat multiples", exact, shots.len() - 1));
            suspicion += 1.0;
        }
    }
    Integrity { flags, suspicion, hits_checked: hits }
}
