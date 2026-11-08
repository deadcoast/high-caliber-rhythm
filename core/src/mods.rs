//! Mods, osu style. Each changes the chart or the rules and carries a score
//! multiplier; the set is part of the replay and of the leaderboard key, so
//! a DT play never competes with a nomod one.
//!
//! DT and HT work through the BPM dial: the chart is beat-space, so ×1.5 BPM
//! is the same level, faster, with the same windows in milliseconds.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Mods {
    /// Double time: BPM × 1.5.
    pub dt: bool,
    /// Half time: BPM × 0.75.
    pub ht: bool,
    /// Hard rock: weak points at 75% radius.
    pub hr: bool,
    /// Hidden: no approach rings (engine-side; recorded here for the key).
    pub hd: bool,
    /// No fail: downs reset HP without restarting the section.
    pub nf: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { dt: false, ht: false, hr: false, hd: false, nf: false };

    pub fn bpm_factor(&self) -> f64 {
        if self.dt && !self.ht {
            1.5
        } else if self.ht && !self.dt {
            0.75
        } else {
            1.0
        }
    }

    pub fn radius_factor(&self) -> f32 {
        if self.hr { 0.75 } else { 1.0 }
    }

    /// Score multiplier, multiplicative across mods.
    pub fn multiplier(&self) -> f64 {
        let mut m = 1.0;
        if self.dt && !self.ht {
            m *= 1.12;
        }
        if self.ht && !self.dt {
            m *= 0.30;
        }
        if self.hr {
            m *= 1.06;
        }
        if self.hd {
            m *= 1.06;
        }
        if self.nf {
            m *= 0.50;
        }
        m
    }

    /// "DTHR", "NM" for none. Order is fixed so the label is a stable key.
    pub fn label(&self) -> String {
        let mut s = String::new();
        if self.dt && !self.ht {
            s.push_str("DT");
        }
        if self.ht && !self.dt {
            s.push_str("HT");
        }
        if self.hr {
            s.push_str("HR");
        }
        if self.hd {
            s.push_str("HD");
        }
        if self.nf {
            s.push_str("NF");
        }
        if s.is_empty() {
            s.push_str("NM");
        }
        s
    }

    /// Parse a label; unknown pairs are ignored. "NM" or "" is no mods.
    pub fn from_label(label: &str) -> Mods {
        let mut m = Mods::NONE;
        let up = label.to_ascii_uppercase();
        let bytes = up.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            match &up[i..i + 2] {
                "DT" => m.dt = true,
                "HT" => m.ht = true,
                "HR" => m.hr = true,
                "HD" => m.hd = true,
                "NF" => m.nf = true,
                _ => {}
            }
            i += 2;
        }
        if m.dt && m.ht {
            m.dt = false;
            m.ht = false;
        }
        m
    }
}
