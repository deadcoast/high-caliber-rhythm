//! Conductor: the beat clock.
//!
//! The conductor owns *song time*: seconds since the start of the audio stream.
//! Beat 0 sits at `song_offset_s`. Everything else in the game is expressed in
//! beats and converted through this one object, so a level authored once plays
//! at any BPM.
//!
//! Audio lock: the engine reports the audio stream position each frame. That
//! position is quantised to the mixer chunk (10-20 ms in Godot) so it jitters.
//! The conductor integrates frame time (smooth) and pulls toward the audio
//! clock with a small gain (accurate). A large error snaps instead of pulling,
//! which handles seeks, loops and buffer underruns.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClockConfig {
    pub bpm: f64,
    pub beats_per_bar: u32,
    /// Subdivisions per beat used for tick events and tracking samples (4 = sixteenths).
    pub ticks_per_beat: u32,
    /// Song time at which beat 0 falls.
    pub song_offset_s: f64,
    /// Calibration: added to every measured shot time. Negative compensates a
    /// pipeline that reports hits late.
    pub input_offset_ms: f64,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            bpm: 128.0,
            beats_per_bar: 4,
            ticks_per_beat: 4,
            song_offset_s: 0.0,
            input_offset_ms: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClockEvent {
    /// Global tick index (ticks since beat 0), the beat it belongs to and the
    /// subdivision within that beat.
    Tick { index: i64, beat: i64, sub: u32 },
    /// Whole beat. `in_bar` is 0-based position in the bar.
    Beat { index: i64, in_bar: u32 },
    Bar { index: i64 },
}

/// Pull gain per frame when the audio clock disagrees by less than `SNAP_S`.
pub const PULL_GAIN: f64 = 0.12;
/// Errors above this snap the clock instead of pulling.
pub const SNAP_S: f64 = 0.050;

#[derive(Clone, Debug)]
pub struct Conductor {
    cfg: ClockConfig,
    song_time: f64,
    running: bool,
    last_tick: i64,
    drift_ms: f64,
    snaps: u32,
    ticks_emitted: u64,
}

impl Conductor {
    pub fn new(cfg: ClockConfig) -> Self {
        assert!(cfg.bpm > 0.0, "bpm must be positive");
        assert!(cfg.ticks_per_beat >= 1 && cfg.beats_per_bar >= 1);
        Self {
            cfg,
            song_time: 0.0,
            running: false,
            last_tick: 0,
            drift_ms: 0.0,
            snaps: 0,
            ticks_emitted: 0,
        }
    }

    pub fn config(&self) -> &ClockConfig {
        &self.cfg
    }

    pub fn set_input_offset_ms(&mut self, ms: f64) {
        self.cfg.input_offset_ms = ms;
    }

    pub fn sec_per_beat(&self) -> f64 {
        60.0 / self.cfg.bpm
    }

    pub fn sec_per_tick(&self) -> f64 {
        self.sec_per_beat() / self.cfg.ticks_per_beat as f64
    }

    pub fn beat_to_time(&self, beat: f64) -> f64 {
        self.cfg.song_offset_s + beat * self.sec_per_beat()
    }

    pub fn time_to_beat(&self, t: f64) -> f64 {
        (t - self.cfg.song_offset_s) / self.sec_per_beat()
    }

    /// Start the clock at a song time. Negative times (pre-roll before the
    /// stream starts) are allowed; events before beat 0 carry negative indices.
    pub fn start(&mut self, song_time: f64) {
        self.song_time = song_time;
        self.running = true;
        self.last_tick = self.tick_index_at(song_time);
        self.drift_ms = 0.0;
        self.snaps = 0;
        self.ticks_emitted = 0;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Jump to a song time without emitting events (replay, editor scrub).
    /// Only moves forward; an earlier time is ignored.
    pub fn seek_forward(&mut self, song_time: f64) {
        if song_time > self.song_time {
            self.song_time = song_time;
            self.last_tick = self.tick_index_at(song_time);
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn song_time(&self) -> f64 {
        self.song_time
    }

    pub fn song_beat(&self) -> f64 {
        self.time_to_beat(self.song_time)
    }

    /// 0..1 progress through the current beat. Drives the crosshair pulse.
    pub fn beat_phase(&self) -> f64 {
        let b = self.song_beat();
        b - b.floor()
    }

    /// Signed distance in milliseconds from the nearest beat (negative = early).
    pub fn ms_from_nearest_beat(&self, song_time: f64) -> f64 {
        let b = self.time_to_beat(song_time);
        (b - b.round()) * self.sec_per_beat() * 1000.0
    }

    /// Last measured disagreement between the integrated clock and the audio clock.
    pub fn drift_ms(&self) -> f64 {
        self.drift_ms
    }

    pub fn snap_count(&self) -> u32 {
        self.snaps
    }

    pub fn ticks_emitted(&self) -> u64 {
        self.ticks_emitted
    }

    /// Convert a raw measured input time (song time at the moment the engine
    /// saw the click) into judged shot time by applying calibration.
    pub fn shot_time(&self, raw_song_time: f64) -> f64 {
        raw_song_time + self.cfg.input_offset_ms / 1000.0
    }

    fn tick_index_at(&self, song_time: f64) -> i64 {
        let ticks = self.time_to_beat(song_time) * self.cfg.ticks_per_beat as f64;
        ticks.floor() as i64
    }

    /// Advance the clock by one frame. `audio_time` is the engine's estimate of
    /// the stream position (already corrected for output latency) or `None`
    /// when no audio clock is available (menus, headless, pre-roll).
    /// Crossed ticks, beats and bars are appended to `out` in order.
    pub fn update(&mut self, dt: f64, audio_time: Option<f64>, out: &mut Vec<ClockEvent>) {
        if !self.running {
            return;
        }
        let predicted = self.song_time + dt;
        match audio_time {
            Some(at) => {
                let err = at - predicted;
                self.drift_ms = err * 1000.0;
                if err.abs() > SNAP_S {
                    self.song_time = at;
                    self.snaps += 1;
                } else {
                    self.song_time = predicted + err * PULL_GAIN;
                }
            }
            None => self.song_time = predicted,
        }
        let now_tick = self.tick_index_at(self.song_time);
        if now_tick < self.last_tick {
            // Clock moved backwards (snap after a seek). Re-anchor silently.
            self.last_tick = now_tick;
            return;
        }
        let tpb = self.cfg.ticks_per_beat as i64;
        let bpb = self.cfg.beats_per_bar as i64;
        for t in (self.last_tick + 1)..=now_tick {
            let beat = t.div_euclid(tpb);
            let sub = t.rem_euclid(tpb) as u32;
            out.push(ClockEvent::Tick { index: t, beat, sub });
            self.ticks_emitted += 1;
            if sub == 0 {
                let in_bar = beat.rem_euclid(bpb) as u32;
                out.push(ClockEvent::Beat { index: beat, in_bar });
                if in_bar == 0 {
                    out.push(ClockEvent::Bar { index: beat.div_euclid(bpb) });
                }
            }
        }
        self.last_tick = now_tick;
    }
}
