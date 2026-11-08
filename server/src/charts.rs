//! The ranked chart registry. A score is ranked when the replay's chart
//! fingerprint is one the server computed itself: every shipped level file
//! (any mod combination that changes the chart) and the builtin levels at
//! every whole BPM from 60 to 220. Anything else is kept on its own board
//! and marked unranked, so a custom level or an Any Song chart still has a
//! leaderboard among the people who play that exact chart.

use downbeat_core::chart::{compile, Level};
use downbeat_core::mods::Mods;
use downbeat_core::patterns::{builtin_lib, fill_level, street_hard_level, street_level, warmup_level};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ChartMeta {
    pub name: String,
    pub bpm: f64,
    pub mods: String,
    /// Shipped level file (campaign, ranked for the daily pool too) or builtin.
    pub shipped: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Charts {
    pub ranked: HashMap<u64, ChartMeta>,
    /// Shipped levels by name, for the daily pool and the client's level fetch.
    pub levels: HashMap<String, Level>,
}

/// The chart-changing mod combinations: DT/HT/none × HR/none. HD and NF
/// leave the chart alone, so their fingerprints are the same.
pub fn chart_mods() -> Vec<Mods> {
    let mut out = Vec::new();
    for (dt, ht) in [(false, false), (true, false), (false, true)] {
        for hr in [false, true] {
            out.push(Mods { dt, ht, hr, hd: false, nf: false });
        }
    }
    out
}

pub fn fingerprint_with_mods(level: &Level, mods: Mods) -> Option<u64> {
    let mut modded = level.clone();
    modded.bpm *= mods.bpm_factor();
    let mut chart = compile(&modded, &builtin_lib()).ok()?;
    if mods.hr {
        for t in &mut chart.targets {
            t.radius *= mods.radius_factor();
        }
    }
    Some(chart.fingerprint())
}

impl Charts {
    /// Build the registry: `level_dir` holds the shipped `*.json` levels
    /// (missing or empty is fine; the builtins are always there).
    pub fn build(level_dir: &str) -> Charts {
        let mut c = Charts::default();
        if let Ok(rd) = std::fs::read_dir(level_dir) {
            let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json")).collect();
            files.sort();
            for path in files {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Ok(level) = Level::from_json(&text) else { continue };
                c.add_level(&level, true);
                c.levels.insert(level.name.clone(), level);
            }
        }
        for bpm in 60..=220 {
            let bpm = bpm as f64;
            for level in [street_level(bpm), street_hard_level(bpm), warmup_level(bpm), fill_level(bpm)] {
                c.add_level(&level, false);
            }
        }
        c
    }

    fn add_level(&mut self, level: &Level, shipped: bool) {
        for m in chart_mods() {
            if let Some(fp) = fingerprint_with_mods(level, m) {
                self.ranked.entry(fp).or_insert(ChartMeta { name: level.name.clone(), bpm: level.bpm, mods: m.label(), shipped });
            }
        }
    }

    pub fn get(&self, fingerprint: u64) -> Option<&ChartMeta> {
        self.ranked.get(&fingerprint)
    }
}
