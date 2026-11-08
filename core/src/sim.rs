//! Headless simulation: a bot with configurable timing and aim error plays a
//! chart through the real `Game` object with a synthetic, chunk-quantised
//! audio clock. This is how the model is verified without an engine.

use crate::chart::{Kind, Level};
use crate::clock::{ClockConfig, ClockEvent};
use crate::judge::{Hit, Verdict, Windows};
use crate::score::{Report, Scoring};
use crate::{Game, GameEvent};

/// Deterministic RNG (SplitMix64) so every run is reproducible from a seed.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal via Box-Muller.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-12);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bot {
    /// Systematic lateness in ms (positive = late).
    pub bias_ms: f64,
    /// Standard deviation of timing error in ms.
    pub jitter_ms: f64,
    /// Probability a shot lands on its target.
    pub hit_prob: f64,
    /// Landing offset as a fraction of radius, uniform in [0, spread].
    pub spread: f64,
    /// Probability of never shooting a target.
    pub skip_prob: f64,
    /// Probability each tracking sample is on target.
    pub track_on_prob: f64,
}

impl Bot {
    pub fn perfect() -> Self {
        Self { bias_ms: 0.0, jitter_ms: 0.0, hit_prob: 1.0, spread: 0.0, skip_prob: 0.0, track_on_prob: 1.0 }
    }

    pub fn human(jitter_ms: f64) -> Self {
        Self { bias_ms: 8.0, jitter_ms, hit_prob: 0.92, spread: 0.8, skip_prob: 0.02, track_on_prob: 0.85 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimConfig {
    pub seed: u64,
    /// Frame step in seconds (1/240 mimics an uncapped high-refresh loop).
    pub frame_dt: f64,
    /// Audio clock chunk size in seconds (Godot default mixer ~ 512 samples @ 44.1 kHz).
    pub audio_chunk_s: f64,
    /// Whether to feed an audio clock at all.
    pub audio_clock: bool,
    /// Pre-roll before beat 0 in seconds.
    pub preroll_s: f64,
    /// Wall-clock cap in simulated seconds; restarts can otherwise loop forever.
    pub max_wall_s: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self { seed: 7, frame_dt: 1.0 / 240.0, audio_chunk_s: 512.0 / 44_100.0, audio_clock: true, preroll_s: 1.0, max_wall_s: 1800.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    pub name: String,
    pub report: Report,
    pub frames: u64,
    pub ticks: u64,
    pub beats: u64,
    pub bars: u64,
    pub spawns: u32,
    pub snaps: u32,
    pub restarts: u32,
    pub max_abs_drift_ms: f64,
    pub rms_drift_ms: f64,
    pub verdicts: Vec<Verdict>,
    pub sim_time_s: f64,
    pub replay: Option<crate::replay::Replay>,
}

#[derive(Clone, Copy, Debug)]
struct Plan {
    time: f64,
    fired: bool,
    skip: bool,
}

pub fn run_level(level: &Level, bot: Bot, cfg: SimConfig) -> SimResult {
    let game = Game::from_level(level, ClockConfig::default(), Windows::default(), Scoring::default())
        .expect("level compiles");
    run_game(game, bot, cfg)
}

pub fn run_game(mut game: Game, bot: Bot, cfg: SimConfig) -> SimResult {
    let mut rng = Rng::new(cfg.seed);
    let n = game.chart().targets.len();
    let mut plans: Vec<Plan> = (0..n)
        .map(|i| {
            let t = &game.chart().targets[i];
            let err = bot.bias_ms + bot.jitter_ms * rng.normal();
            Plan {
                time: t.hit_time + err / 1000.0,
                fired: t.kind == Kind::Track,
                skip: t.kind != Kind::Track && rng.unit() < bot.skip_prob,
            }
        })
        .collect();
    let total = game.chart().total_time + 0.5;
    let mut real: f64 = -cfg.preroll_s;
    game.start(real);

    let mut frames = 0u64;
    let mut restarts = 0u32;
    let mut wall = 0.0f64;
    let (mut ticks, mut beats, mut bars, mut spawns) = (0u64, 0u64, 0u64, 0u32);
    let mut live_tracks: Vec<u32> = Vec::new();
    let mut verdicts = Vec::new();
    let mut drift_sq = 0.0;
    let mut drift_n = 0.0;
    let mut max_drift: f64 = 0.0;

    // Restarts rewind `real`; cap the wall clock so a bot that never kills ends.
    while real < total && wall < cfg.max_wall_s {
        let prev = real;
        real += cfg.frame_dt;
        wall += cfg.frame_dt;
        frames += 1;
        let audio = if cfg.audio_clock && real >= 0.0 {
            // Mixer reports the last completed chunk plus a sub-chunk estimate
            // with up to 2 ms of jitter, the way get_playback_position +
            // time_since_last_mix behaves.
            let chunk = (real / cfg.audio_chunk_s).floor() * cfg.audio_chunk_s;
            let sub = (real - chunk) + (rng.unit() - 0.5) * 0.004;
            Some(chunk + sub.max(0.0))
        } else {
            None
        };
        let mut tick_hits = Vec::new();
        let frame_events: Vec<GameEvent> = game.update(cfg.frame_dt, audio).collect();
        for ev in frame_events {
            match ev {
                GameEvent::Clock(ClockEvent::Tick { .. }) => {
                    ticks += 1;
                    tick_hits.push(());
                }
                GameEvent::Clock(ClockEvent::Beat { .. }) => beats += 1,
                GameEvent::Clock(ClockEvent::Bar { .. }) => bars += 1,
                GameEvent::Cue(_) => {}
                GameEvent::Judge(v) => {
                    match &v {
                        Verdict::Spawn { target } => {
                            spawns += 1;
                            if game.target(*target).map(|t| t.kind) == Some(Kind::Track) {
                                live_tracks.push(*target);
                            }
                        }
                        Verdict::TrackDone { target, .. } => live_tracks.retain(|t| t != target),
                        Verdict::Restart { segment, .. } => {
                            // The section replays: the bot gets another go at
                            // every target in it, with fresh error and fresh luck.
                            live_tracks.clear();
                            for (i, p) in plans.iter_mut().enumerate() {
                                let t = &game.chart().targets[i];
                                if t.segment == *segment {
                                    let err = bot.bias_ms + bot.jitter_ms * rng.normal();
                                    p.time = t.hit_time + err / 1000.0;
                                    p.fired = t.kind == Kind::Track;
                                    p.skip = t.kind != Kind::Track && rng.unit() < bot.skip_prob;
                                }
                            }
                            // Real time continues; chart time went back. Track the model's clock.
                            real = game.conductor().song_time();
                            restarts += 1;
                        }
                        _ => {}
                    }
                    verdicts.push(v);
                }
            }
        }
        for _ in tick_hits {
            for &t in &live_tracks {
                game.track_sample(t, rng.unit() < bot.track_on_prob);
            }
        }
        if audio.is_some() {
            let d = game.conductor().drift_ms();
            drift_sq += d * d;
            drift_n += 1.0;
            max_drift = max_drift.max(d.abs());
        }
        // Fire every shot whose planned time fell inside this frame. The shot
        // carries its exact planned stream time, as an input timestamp would.
        for (i, p) in plans.iter_mut().enumerate() {
            if p.fired || p.skip {
                continue;
            }
            if p.time > prev && p.time <= real {
                p.fired = true;
                let target = &game.chart().targets[i];
                let hit = if rng.unit() < bot.hit_prob {
                    Some(Hit { target: target.id, offset_m: (target.radius as f64 * bot.spread * rng.unit()) as f32 })
                } else {
                    None
                };
                let v = game.fire(p.time, hit);
                verdicts.push(v);
            }
        }
    }
    SimResult {
        name: game.chart().name.clone(),
        report: game.report(),
        frames,
        ticks,
        beats,
        bars,
        spawns,
        snaps: game.conductor().snap_count(),
        restarts,
        max_abs_drift_ms: max_drift,
        rms_drift_ms: if drift_n > 0.0 { (drift_sq / drift_n).sqrt() } else { 0.0 },
        verdicts,
        sim_time_s: real,
        replay: game.replay(),
    }
}
