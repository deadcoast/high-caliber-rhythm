//! Timing and aim judgement.
//!
//! Windows are in milliseconds, never beats: human timing resolution does not
//! change with tempo, so a Perfect at 90 BPM is the same 30 ms as at 174. What
//! changes with BPM is how many of those windows per second the chart demands.
//!
//! All times here are chart times: seconds since beat 0. The conductor converts
//! stream time to chart time before anything reaches the judge.

use crate::chart::{Chart, Kind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Windows {
    pub perfect_ms: f64,
    pub great_ms: f64,
    pub good_ms: f64,
    /// A target stays shootable this long after its beat. A shot between
    /// `good_ms` and `late_ms` is LATE (consumed, timing miss); past `late_ms`
    /// the target expires on its own.
    pub late_ms: f64,
}

impl Default for Windows {
    fn default() -> Self {
        Self { perfect_ms: 30.0, great_ms: 60.0, good_ms: 100.0, late_ms: 160.0 }
    }
}

impl Windows {
    pub fn judge(&self, delta_ms: f64) -> Timing {
        let a = delta_ms.abs();
        if a <= self.perfect_ms {
            Timing::Perfect
        } else if a <= self.great_ms {
            Timing::Great
        } else if a <= self.good_ms {
            Timing::Good
        } else if delta_ms < 0.0 {
            Timing::Early
        } else {
            Timing::Late
        }
    }

    pub fn good_s(&self) -> f64 {
        self.good_ms / 1000.0
    }

    pub fn late_s(&self) -> f64 {
        self.late_ms / 1000.0
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Timing {
    Perfect,
    Great,
    Good,
    /// Shot landed, too early. Counts as a timing miss; the target is consumed.
    Early,
    /// Shot landed, too late. Counts as a timing miss; the target is consumed.
    Late,
}

impl Timing {
    pub fn is_on_beat(self) -> bool {
        matches!(self, Timing::Perfect | Timing::Great | Timing::Good)
    }

    pub fn label(self) -> &'static str {
        match self {
            Timing::Perfect => "PERFECT",
            Timing::Great => "GREAT",
            Timing::Good => "GOOD",
            Timing::Early => "EARLY",
            Timing::Late => "LATE",
        }
    }
}

/// A raycast result the engine hands the judge: which target, and how far from
/// its centre the ray landed, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub target: u32,
    pub offset_m: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    /// Calibrated chart time of the click.
    pub time: f64,
    pub hit: Option<Hit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Verdict {
    /// The engine should create this target's visual now.
    Spawn { target: u32 },
    Hit {
        target: u32,
        kind: Kind,
        timing: Timing,
        delta_ms: f64,
        /// 1.0 = dead centre, 0.0 = rim.
        precision: f32,
        /// The beat the target was charted on; results scatter plots use it.
        hit_beat: f64,
    },
    /// Shot hit no live target. `grid_delta_ms` is the distance to the nearest
    /// beat so the HUD can still show the player where the pulse was.
    Whiff { grid_delta_ms: f64 },
    /// Target left its window unshot.
    Expired { target: u32, kind: Kind },
    /// Tracking target finished: `on` of `total` tick samples were on target.
    TrackDone { target: u32, on: u32, total: u32 },
    /// Shot touched a target that is not judgeable (already consumed, tracking).
    Ignored { target: u32, reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Pending,
    Live,
    Tracking,
    Done,
}

#[derive(Clone, Debug)]
pub struct Judge {
    chart: Chart,
    windows: Windows,
    state: Vec<State>,
    track_on: Vec<u32>,
    track_total: Vec<u32>,
    next_spawn: usize,
    /// Lowest index that may still be Live or Tracking; everything below is Done.
    low_water: usize,
    now: f64,
}

impl Judge {
    pub fn new(chart: Chart, windows: Windows) -> Self {
        let n = chart.targets.len();
        Self {
            chart,
            windows,
            state: vec![State::Pending; n],
            track_on: vec![0; n],
            track_total: vec![0; n],
            next_spawn: 0,
            low_water: 0,
            now: f64::NEG_INFINITY,
        }
    }

    pub fn chart(&self) -> &Chart {
        &self.chart
    }

    pub fn windows(&self) -> Windows {
        self.windows
    }

    pub fn set_windows(&mut self, w: Windows) {
        self.windows = w;
    }

    pub fn sec_per_beat(&self) -> f64 {
        60.0 / self.chart.bpm
    }

    pub fn ms_from_nearest_beat(&self, chart_time: f64) -> f64 {
        let b = chart_time / self.sec_per_beat();
        (b - b.round()) * self.sec_per_beat() * 1000.0
    }

    pub fn is_live(&self, target: u32) -> bool {
        matches!(self.state.get(target as usize), Some(State::Live) | Some(State::Tracking))
    }

    pub fn is_finished(&self) -> bool {
        self.state.iter().all(|s| *s == State::Done)
    }

    pub fn live_count(&self) -> usize {
        self.state[self.low_water..]
            .iter()
            .filter(|s| matches!(s, State::Live | State::Tracking))
            .count()
    }

    /// Advance chart time. Emits spawns, expiries and finished tracks.
    /// Time never moves backwards: an earlier value is ignored.
    pub fn advance(&mut self, chart_time: f64, out: &mut Vec<Verdict>) {
        if chart_time < self.now {
            return;
        }
        self.now = chart_time;
        let late = self.windows.late_s();
        while self.next_spawn < self.chart.spawn_order.len() {
            let idx = self.chart.spawn_order[self.next_spawn];
            if self.chart.targets[idx].spawn_time > chart_time {
                break;
            }
            if self.state[idx] == State::Pending {
                self.state[idx] = State::Live;
                out.push(Verdict::Spawn { target: idx as u32 });
            }
            self.next_spawn += 1;
        }
        for i in self.low_water..self.chart.targets.len() {
            let t = &self.chart.targets[i];
            if t.hit_time - late > chart_time {
                break;
            }
            match (self.state[i], t.kind) {
                (State::Live, Kind::Track) if chart_time >= t.hit_time => {
                    self.state[i] = State::Tracking;
                }
                (State::Tracking, Kind::Track) if chart_time > t.end_time => {
                    self.state[i] = State::Done;
                    out.push(Verdict::TrackDone {
                        target: i as u32,
                        on: self.track_on[i],
                        total: self.track_total[i],
                    });
                }
                (State::Live, _) if chart_time > t.hit_time + late => {
                    self.state[i] = State::Done;
                    out.push(Verdict::Expired { target: i as u32, kind: t.kind });
                }
                _ => {}
            }
        }
        while self.low_water < self.state.len() && self.state[self.low_water] == State::Done {
            self.low_water += 1;
        }
    }

    /// Judge a click.
    pub fn fire(&mut self, shot: Shot) -> Verdict {
        let Some(hit) = shot.hit else {
            return Verdict::Whiff { grid_delta_ms: self.ms_from_nearest_beat(shot.time) };
        };
        let i = hit.target as usize;
        if i >= self.chart.targets.len() {
            return Verdict::Ignored { target: hit.target, reason: "unknown target".into() };
        }
        let t = &self.chart.targets[i];
        match self.state[i] {
            State::Live if t.kind != Kind::Track => {
                let delta_ms = (shot.time - t.hit_time) * 1000.0;
                let timing = self.windows.judge(delta_ms);
                let precision = (1.0 - hit.offset_m / t.radius.max(1e-4)).clamp(0.0, 1.0);
                self.state[i] = State::Done;
                Verdict::Hit { target: hit.target, kind: t.kind, timing, delta_ms, precision, hit_beat: t.hit_beat }
            }
            State::Live | State::Tracking => {
                Verdict::Ignored { target: hit.target, reason: "track target: hold, do not click".into() }
            }
            State::Pending => Verdict::Ignored { target: hit.target, reason: "not spawned".into() },
            State::Done => Verdict::Ignored { target: hit.target, reason: "already consumed".into() },
        }
    }

    /// One tracking sample. Call once per clock tick for every live Track
    /// target with whether the crosshair was on it while firing. Samples
    /// outside the hold window are discarded.
    pub fn track_sample(&mut self, target: u32, on: bool) -> bool {
        let i = target as usize;
        if i >= self.state.len() || self.state[i] != State::Tracking {
            return false;
        }
        self.track_total[i] += 1;
        if on {
            self.track_on[i] += 1;
        }
        true
    }
}
