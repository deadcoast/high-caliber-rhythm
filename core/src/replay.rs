//! Replays. The model is deterministic, so a run is fully described by its
//! level, clock settings and the ordered inputs. Replaying produces the same
//! report, which is what a leaderboard validates and what a ghost or spectator
//! view plays back.
//!
//! Every input carries `at`: the chart time the judge had been advanced to
//! when the input was applied live. Replay advances to the same `at` before
//! applying it, so expiries, spawns and tracking state are identical whatever
//! the live frame timing was.

use crate::chart::Level;
use crate::clock::ClockConfig;
use crate::judge::{Hit, Windows};
use crate::score::{Report, Scoring};
use crate::Game;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Input {
    /// A click: raw stream time as the engine measured it (before calibration),
    /// the target the ray hit (-1 = none) and ray-to-centre distance.
    Shot { at: f64, raw_time: f64, target: i64, offset_m: f32 },
    /// A tracking sample.
    Track { at: f64, target: u32, on: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Replay {
    pub level: Level,
    pub clock: ClockConfig,
    pub windows: Windows,
    pub scoring: Scoring,
    /// `Chart::fingerprint` of the chart the run was judged against.
    pub chart_fingerprint: u64,
    pub inputs: Vec<Input>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    Compile(crate::chart::CompileError),
    /// The level compiles to a different chart than the one the run was played on.
    ChartMismatch { recorded: u64, current: u64 },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Compile(e) => write!(f, "{e}"),
            ReplayError::ChartMismatch { recorded, current } => {
                write!(f, "chart changed since the run: recorded {recorded:016x}, current {current:016x}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

impl Replay {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("replay serialises")
    }

    pub fn from_json(s: &str) -> Result<Replay, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Replay to a report. Refuses if the level no longer compiles to the
    /// chart the run was judged against.
    pub fn play(&self) -> Result<Report, ReplayError> {
        let mut game = Game::from_level(&self.level, self.clock, self.windows, self.scoring).map_err(ReplayError::Compile)?;
        let current = game.chart().fingerprint();
        if current != self.chart_fingerprint {
            return Err(ReplayError::ChartMismatch { recorded: self.chart_fingerprint, current });
        }
        game.start(-1.0);
        for input in &self.inputs {
            match *input {
                Input::Shot { at, raw_time, target, offset_m } => {
                    game.seek_chart_time(at);
                    let hit = if target >= 0 { Some(Hit { target: target as u32, offset_m }) } else { None };
                    game.fire(raw_time, hit);
                }
                Input::Track { at, target, on } => {
                    game.seek_chart_time(at);
                    game.track_sample(target, on);
                }
            }
        }
        game.seek_chart_time(game.chart().total_time + 1.0);
        Ok(game.report())
    }
}
