//! Difficulty rating for a compiled chart.
//!
//! The chart is read the way a player's aim experiences it: for each target,
//! how far the crosshair must travel (degrees from the previous target), how
//! much time there is to do it (the shorter of the gap to the previous beat
//! and the target's visible lead), and how small the target is in angular
//! terms. Tracking targets contribute their angular speed against their size.
//! Peaks dominate, as in every rhythm-game rating that players trust: the
//! number reports the hard passages, not the average bar.

use crate::chart::{Aim, Chart, Kind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rating {
    /// Headline number, open-ended; the warmup at 128 BPM is the 3.0 anchor.
    pub stars: f64,
    /// Peak aim demand, degrees per second over the hardest fifth of targets.
    pub aim_speed: f64,
    /// Peak precision demand, 1 / angular radius in degrees, same window.
    pub precision: f64,
    /// Targets per second over the whole chart.
    pub density: f64,
    /// Tracking demand, degrees per second of path over angular radius.
    pub tracking: f64,
    /// Per-target strain in chart order (non-track), for a difficulty graph.
    pub strain: Vec<f64>,
}

fn angle_between(a: &Aim, b: &Aim) -> f64 {
    let da = a.direction();
    let db = b.direction();
    let dot = (da[0] * db[0] + da[1] * db[1] + da[2] * db[2]).clamp(-1.0, 1.0);
    (dot as f64).acos().to_degrees()
}

fn angular_radius_deg(radius: f32, dist: f32) -> f64 {
    ((radius / dist.max(0.01)) as f64).atan().to_degrees()
}

/// Mean of the top `frac` of values (at least one).
fn peak_mean(values: &[f64], frac: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let n = ((v.len() as f64 * frac).ceil() as usize).max(1).min(v.len());
    v[..n].iter().sum::<f64>() / n as f64
}

pub fn rate(chart: &Chart) -> Rating {
    let spb = 60.0 / chart.bpm;
    let targets: Vec<_> = chart.targets.iter().filter(|t| t.kind != Kind::Track).collect();
    let mut strain = Vec::with_capacity(targets.len());
    let mut speeds = Vec::new();
    let mut precisions = Vec::new();
    // The first target is approached from lane centre.
    let centre = Aim::new(0.0, 0.0, 12.0);
    let mut prev_aim = &centre;
    let mut prev_time = chart.targets.first().map(|t| t.spawn_time).unwrap_or(0.0);
    for t in &targets {
        let travel = angle_between(prev_aim, &t.aim);
        let gap = (t.hit_time - prev_time).max(0.05);
        let lead = (t.hit_time - t.spawn_time).max(0.05);
        let avail = gap.min(lead);
        let ang_r = angular_radius_deg(t.radius, t.aim.dist).max(0.05);
        // Fitts' index of difficulty in bits: log2(distance / width + 1), with
        // width the target's angular diameter. Strain is bits per second, the
        // aim-trainer throughput demand of this shot.
        let bits = (travel / (2.0 * ang_r) + 1.0).log2();
        strain.push(bits / avail);
        speeds.push(travel / avail);
        precisions.push(1.0 / ang_r);
        prev_aim = &t.aim;
        prev_time = t.hit_time;
    }
    let mut tracking = Vec::new();
    for t in chart.targets.iter().filter(|t| t.kind == Kind::Track) {
        if let Some(to) = &t.to {
            let path = angle_between(&t.aim, to);
            let dur = (t.end_time - t.hit_time).max(0.05);
            let ang_r = angular_radius_deg(t.radius, t.aim.dist).max(0.05);
            tracking.push(path / dur / ang_r);
        }
    }
    let total_time = chart.total_time.max(spb);
    let density = chart.targets.len() as f64 / total_time;
    let aim_speed = peak_mean(&speeds, 0.2);
    let precision = peak_mean(&precisions, 0.2);
    let track = peak_mean(&tracking, 0.5);
    let peak_strain = peak_mean(&strain, 0.2);
    let raw = peak_strain + 2.0 * density + 0.5 * track;
    let stars = raw * 3.0 / WARMUP_128_RAW;
    Rating { stars, aim_speed, precision, density, tracking: track, strain }
}

/// `raw` of the builtin warmup at 128 BPM; fixes the 3.0-star anchor. The
/// `rating_anchor` test holds it to the current pattern library.
pub const WARMUP_128_RAW: f64 = 20.222;
