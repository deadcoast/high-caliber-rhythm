//! Scoring and run statistics.
//!
//! Two axes, both reported: WHEN (timing) and WHERE (aim). Score needs both;
//! the stats keep them apart so a player can see which one is holding them
//! back. Combo rewards sustained on-beat hitting and breaks on any miss.

use crate::chart::Kind;
use crate::judge::{Timing, Verdict};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Scoring {
    pub base: f64,
    pub perfect_mult: f64,
    pub great_mult: f64,
    pub good_mult: f64,
    /// Fraction of shot score that is unconditional; the rest scales with precision.
    pub precision_floor: f64,
    /// Combo count at which the combo multiplier reaches `max_combo_mult`.
    pub combo_cap: u32,
    pub max_combo_mult: f64,
    /// Points per track sample on target.
    pub track_sample_points: f64,
}

impl Default for Scoring {
    fn default() -> Self {
        Self {
            base: 100.0,
            perfect_mult: 1.0,
            great_mult: 0.75,
            good_mult: 0.5,
            precision_floor: 0.6,
            combo_cap: 50,
            max_combo_mult: 2.0,
            track_sample_points: 25.0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct KindStats {
    pub targets: u32,
    pub hits: u32,
    pub on_beat: u32,
    pub expired: u32,
    pub precision_sum: f64,
    pub track_on: u32,
    pub track_total: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Grade {
    S,
    A,
    B,
    C,
    D,
    F,
}

impl Grade {
    pub fn label(self) -> &'static str {
        match self {
            Grade::S => "S",
            Grade::A => "A",
            Grade::B => "B",
            Grade::C => "C",
            Grade::D => "D",
            Grade::F => "F",
        }
    }
}

/// One judged shot, in order. The results screen's timing scatter is this list.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ShotRecord {
    pub target: u32,
    pub kind: Kind,
    pub timing: Timing,
    pub hit_beat: f64,
    pub delta_ms: f64,
    pub precision: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub score: u64,
    pub max_combo: u32,
    pub shots: u32,
    pub hits: u32,
    pub whiffs: u32,
    pub expired: u32,
    pub perfect: u32,
    pub great: u32,
    pub good: u32,
    pub early: u32,
    pub late: u32,
    /// Spatial: hits / shots.
    pub aim_accuracy: f64,
    /// Mean precision over hits (1 = centre).
    pub precision: f64,
    /// Timing: on-beat hits / judged targets (hits + expired).
    pub beat_accuracy: f64,
    /// Weighted timing quality: perfect 1, great .75, good .5 over judged targets.
    pub timing_quality: f64,
    pub timing_mean_ms: f64,
    pub timing_std_ms: f64,
    pub track_ratio: f64,
    pub grade: Grade,
    pub by_kind: BTreeMap<String, KindStats>,
    pub history: Vec<ShotRecord>,
}

#[derive(Clone, Debug)]
pub struct Session {
    scoring: Scoring,
    score: f64,
    combo: u32,
    max_combo: u32,
    shots: u32,
    hits: u32,
    whiffs: u32,
    expired: u32,
    counts: [u32; 5],
    precision_sum: f64,
    offsets: Vec<f64>,
    track_on: u32,
    track_total: u32,
    by_kind: BTreeMap<Kind, KindStats>,
    last_delta_ms: f64,
    history: Vec<ShotRecord>,
}

impl Session {
    pub fn new(scoring: Scoring) -> Self {
        Self {
            scoring,
            score: 0.0,
            combo: 0,
            max_combo: 0,
            shots: 0,
            hits: 0,
            whiffs: 0,
            expired: 0,
            counts: [0; 5],
            precision_sum: 0.0,
            offsets: Vec::new(),
            track_on: 0,
            track_total: 0,
            by_kind: BTreeMap::new(),
            last_delta_ms: 0.0,
            history: Vec::new(),
        }
    }

    pub fn score(&self) -> u64 {
        self.score.round() as u64
    }

    pub fn combo(&self) -> u32 {
        self.combo
    }

    pub fn max_combo(&self) -> u32 {
        self.max_combo
    }

    pub fn last_delta_ms(&self) -> f64 {
        self.last_delta_ms
    }

    pub fn combo_mult(&self) -> f64 {
        let c = self.combo.min(self.scoring.combo_cap) as f64 / self.scoring.combo_cap as f64;
        1.0 + (self.scoring.max_combo_mult - 1.0) * c
    }

    fn timing_index(t: Timing) -> usize {
        match t {
            Timing::Perfect => 0,
            Timing::Great => 1,
            Timing::Good => 2,
            Timing::Early => 3,
            Timing::Late => 4,
        }
    }

    fn timing_mult(&self, t: Timing) -> f64 {
        match t {
            Timing::Perfect => self.scoring.perfect_mult,
            Timing::Great => self.scoring.great_mult,
            Timing::Good => self.scoring.good_mult,
            Timing::Early | Timing::Late => 0.0,
        }
    }

    fn break_combo(&mut self) {
        self.combo = 0;
    }

    fn bump_combo(&mut self) {
        self.combo += 1;
        self.max_combo = self.max_combo.max(self.combo);
    }

    pub fn apply(&mut self, v: &Verdict) {
        match v {
            Verdict::Spawn { .. } | Verdict::Ignored { .. } => {}
            Verdict::Hit { target, kind, timing, delta_ms, precision, hit_beat } => {
                self.history.push(ShotRecord {
                    target: *target,
                    kind: *kind,
                    timing: *timing,
                    hit_beat: *hit_beat,
                    delta_ms: *delta_ms,
                    precision: *precision,
                });
                self.shots += 1;
                self.hits += 1;
                self.counts[Self::timing_index(*timing)] += 1;
                self.precision_sum += *precision as f64;
                self.offsets.push(*delta_ms);
                self.last_delta_ms = *delta_ms;
                let ks = self.by_kind.entry(*kind).or_default();
                ks.targets += 1;
                ks.hits += 1;
                ks.precision_sum += *precision as f64;
                if timing.is_on_beat() {
                    ks.on_beat += 1;
                    self.bump_combo();
                    let p = self.scoring.precision_floor
                        + (1.0 - self.scoring.precision_floor) * *precision as f64;
                    self.score += self.scoring.base * self.timing_mult(*timing) * p * self.combo_mult();
                } else {
                    self.break_combo();
                }
            }
            Verdict::Whiff { .. } => {
                self.shots += 1;
                self.whiffs += 1;
                self.break_combo();
            }
            Verdict::Expired { kind, .. } => {
                self.expired += 1;
                self.by_kind.entry(*kind).or_default().targets += 1;
                self.by_kind.entry(*kind).or_default().expired += 1;
                self.break_combo();
            }
            Verdict::TrackDone { on, total, .. } => {
                self.track_on += on;
                self.track_total += total;
                let ks = self.by_kind.entry(Kind::Track).or_default();
                ks.targets += 1;
                ks.track_on += on;
                ks.track_total += total;
                let ratio = if *total > 0 { *on as f64 / *total as f64 } else { 0.0 };
                if ratio >= 0.5 {
                    ks.hits += 1;
                    ks.on_beat += 1;
                    self.bump_combo();
                } else {
                    self.break_combo();
                }
                self.score += self.scoring.track_sample_points * *on as f64 * self.combo_mult();
            }
        }
    }

    pub fn report(&self) -> Report {
        let judged = (self.hits + self.expired) as f64;
        let on_beat = (self.counts[0] + self.counts[1] + self.counts[2]) as f64;
        let quality = self.counts[0] as f64 * 1.0 + self.counts[1] as f64 * 0.75 + self.counts[2] as f64 * 0.5;
        let n = self.offsets.len() as f64;
        let mean = if n > 0.0 { self.offsets.iter().sum::<f64>() / n } else { 0.0 };
        let var = if n > 1.0 {
            self.offsets.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        let aim_accuracy = if self.shots > 0 { self.hits as f64 / self.shots as f64 } else { 0.0 };
        let beat_accuracy = if judged > 0.0 { on_beat / judged } else { 0.0 };
        let timing_quality = if judged > 0.0 { quality / judged } else { 0.0 };
        let precision = if self.hits > 0 { self.precision_sum / self.hits as f64 } else { 0.0 };
        let track_ratio = if self.track_total > 0 {
            self.track_on as f64 / self.track_total as f64
        } else {
            1.0
        };
        let composite = 0.4 * timing_quality + 0.3 * aim_accuracy + 0.15 * precision + 0.15 * track_ratio;
        let grade = match composite {
            c if c >= 0.95 => Grade::S,
            c if c >= 0.88 => Grade::A,
            c if c >= 0.78 => Grade::B,
            c if c >= 0.65 => Grade::C,
            c if c >= 0.50 => Grade::D,
            _ => Grade::F,
        };
        Report {
            score: self.score(),
            max_combo: self.max_combo,
            shots: self.shots,
            hits: self.hits,
            whiffs: self.whiffs,
            expired: self.expired,
            perfect: self.counts[0],
            great: self.counts[1],
            good: self.counts[2],
            early: self.counts[3],
            late: self.counts[4],
            aim_accuracy,
            precision,
            beat_accuracy,
            timing_quality,
            timing_mean_ms: mean,
            timing_std_ms: var.sqrt(),
            track_ratio,
            grade,
            by_kind: self
                .by_kind
                .iter()
                .map(|(k, v)| (k.label().to_string(), v.clone()))
                .collect(),
            history: self.history.clone(),
        }
    }
}
