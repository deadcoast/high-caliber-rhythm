//! Song analysis: tempo and the time of beat 0 from audio.
//!
//! Pipeline: short-time spectrum (1024-sample Hann window, hop 256) ->
//! spectral flux onset strength -> autocorrelation tempo estimate with
//! harmonic support and a gentle prior around 120 BPM -> comb-filter phase
//! search for the beat grid -> parabolic refinement of both.
//!
//! Output feeds `Level.bpm` and `Level.song_offset_s`. Authoring still checks
//! the result by ear; the calibration run covers the player's own latency.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

pub const WINDOW: usize = 1024;
pub const HOP: usize = 256;
pub const BPM_MIN: f64 = 60.0;
pub const BPM_MAX: f64 = 200.0;
/// Samples between a frame's start and the transient that maximises its flux.
pub const ONSET_LAG_SAMPLES: f64 = 768.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Analysis {
    pub sample_rate: u32,
    pub duration_s: f64,
    pub bpm: f64,
    /// Seconds into the file at which beat 0 (the first grid beat) falls.
    pub offset_s: f64,
    /// Autocorrelation peak relative to the mean over the search range.
    pub tempo_confidence: f64,
    /// Comb score of the chosen phase relative to the mean over all phases.
    pub phase_confidence: f64,
    pub beats: f64,
    pub bars: f64,
    /// Onset strength per frame, for plotting or a chart editor.
    pub onset: Vec<f32>,
    pub frame_rate: f64,
}

/// Onset strength function: positive flux of log band energies.
///
/// Bins are grouped into log-spaced bands so a hi-hat (hundreds of bins) and a
/// kick (a handful of bins) weigh about the same; the lowest bands are then
/// weighted up because in the music this game runs on, the beat lives in the
/// kick and the bass.
pub fn onset_strength(samples: &[f32], sample_rate: u32) -> (Vec<f32>, f64) {
    let frame_rate = sample_rate as f64 / HOP as f64;
    if samples.len() < WINDOW {
        return (Vec::new(), frame_rate);
    }
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW);
    let hann: Vec<f32> = (0..WINDOW)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WINDOW as f32).cos())
        .collect();
    let bin_hz = sample_rate as f64 / WINDOW as f64;
    // Band edges: 40 Hz to 8 kHz, 24 log-spaced bands, at least one bin each.
    let n_bands = 24usize;
    let f_lo = 40.0f64;
    let f_hi = 8000.0f64.min(sample_rate as f64 / 2.0);
    let mut edges: Vec<usize> = (0..=n_bands)
        .map(|b| ((f_lo * (f_hi / f_lo).powf(b as f64 / n_bands as f64)) / bin_hz).round() as usize)
        .collect();
    for b in 1..edges.len() {
        if edges[b] <= edges[b - 1] {
            edges[b] = edges[b - 1] + 1;
        }
    }
    let weights: Vec<f32> = (0..n_bands).map(|b| if b < 6 { 2.0 } else { 1.0 }).collect();
    let frames = (samples.len() - WINDOW) / HOP + 1;
    let mut prev = vec![0.0f32; n_bands];
    let mut buf = vec![Complex::new(0.0f32, 0.0); WINDOW];
    let mut osf = Vec::with_capacity(frames);
    for f in 0..frames {
        let start = f * HOP;
        for i in 0..WINDOW {
            buf[i] = Complex::new(samples[start + i] * hann[i], 0.0);
        }
        fft.process(&mut buf);
        let mut flux = 0.0f32;
        for b in 0..n_bands {
            let mut e = 0.0f32;
            for k in edges[b]..edges[b + 1].min(WINDOW / 2) {
                e += buf[k].norm_sqr();
            }
            let le = (1.0 + 1000.0 * e).ln();
            let d = le - prev[b];
            if d > 0.0 {
                flux += d * weights[b];
            }
            prev[b] = le;
        }
        osf.push(flux);
    }
    (osf, frame_rate)
}

fn moving_mean(x: &[f32], half: usize) -> Vec<f32> {
    let n = x.len();
    let mut out = vec![0.0f32; n];
    let mut acc = 0.0f64;
    let mut lo = 0usize;
    let mut hi = 0usize; // exclusive
    for i in 0..n {
        let want_lo = i.saturating_sub(half);
        let want_hi = (i + half + 1).min(n);
        while hi < want_hi {
            acc += x[hi] as f64;
            hi += 1;
        }
        while lo < want_lo {
            acc -= x[lo] as f64;
            lo += 1;
        }
        out[i] = (acc / (hi - lo) as f64) as f32;
    }
    out
}

/// Parabolic peak interpolation around index `i` of `y`; returns (offset, value).
fn parabolic(y: &[f64], i: usize) -> (f64, f64) {
    if i == 0 || i + 1 >= y.len() {
        return (0.0, y[i]);
    }
    let (a, b, c) = (y[i - 1], y[i], y[i + 1]);
    let denom = a - 2.0 * b + c;
    if denom.abs() < 1e-12 {
        return (0.0, b);
    }
    let d = 0.5 * (a - c) / denom;
    (d, b - 0.25 * (a - c) * d)
}

pub fn analyze(samples: &[f32], sample_rate: u32) -> Analysis {
    let duration_s = samples.len() as f64 / sample_rate as f64;
    let (osf, frame_rate) = onset_strength(samples, sample_rate);
    let n = osf.len();
    let empty = Analysis {
        sample_rate,
        duration_s,
        bpm: 0.0,
        offset_s: 0.0,
        tempo_confidence: 0.0,
        phase_confidence: 0.0,
        beats: 0.0,
        bars: 0.0,
        onset: osf.clone(),
        frame_rate,
    };
    if n < (frame_rate * 4.0) as usize {
        return empty;
    }
    // High-pass the onset curve: subtract a ~1 s local mean, clamp at 0.
    let local = moving_mean(&osf, (frame_rate * 0.5) as usize);
    let hp: Vec<f64> = osf.iter().zip(local.iter()).map(|(a, b)| ((a - b).max(0.0)) as f64).collect();

    // --- tempo, coarse: autocorrelation over the BPM range with harmonic support.
    let lag_min = (60.0 / BPM_MAX * frame_rate).floor() as usize;
    let lag_max = ((60.0 / BPM_MIN * frame_rate).ceil() as usize).min(n / 2);
    let mut ac = vec![0.0f64; lag_max + 1];
    for lag in 1..=lag_max {
        let mut s = 0.0;
        for i in 0..(n - lag) {
            s += hp[i] * hp[i + lag];
        }
        ac[lag] = s / (n - lag) as f64;
    }
    let energy = ac.iter().skip(1).copied().fold(0.0, f64::max).max(1e-12);
    let mut score = vec![0.0f64; lag_max + 1];
    let mut mean_score = 0.0;
    let mut count = 0usize;
    for lag in lag_min..=lag_max {
        let bpm = 60.0 * frame_rate / lag as f64;
        let prior = (-0.5 * (bpm / 120.0).log2().powi(2)).exp();
        let mut s = ac[lag] / energy;
        if 2 * lag <= lag_max {
            s += 0.5 * ac[2 * lag] / energy;
        }
        if lag % 2 == 0 && lag / 2 >= 1 {
            s += 0.25 * ac[lag / 2] / energy;
        }
        score[lag] = s * (0.6 + 0.4 * prior);
        mean_score += score[lag];
        count += 1;
    }
    let mean_score = mean_score / count.max(1) as f64;
    let mut best_lag = lag_min;
    for lag in lag_min..=lag_max {
        if score[lag] > score[best_lag] {
            best_lag = lag;
        }
    }
    let (d, peak) = parabolic(&score, best_lag);
    let coarse_period = best_lag as f64 + d;
    let tempo_confidence = if mean_score > 0.0 { peak / mean_score } else { 0.0 };

    // --- fine: joint (period, phase) search with a comb over the whole song.
    // A period 0.1% off drifts a full frame by bar 20, so the comb score over
    // the song is sharp in period; that sharpness is the precision.
    let comb_at = |period: f64, phi: f64| -> f64 {
        let mut k = 0.0;
        let mut acc = 0.0;
        let mut m = 0usize;
        loop {
            let x = phi + k * period;
            let i = x.floor();
            if i as usize + 1 >= n || i < 0.0 {
                break;
            }
            let frac = x - i;
            let i = i as usize;
            acc += hp[i] * (1.0 - frac) + hp[i + 1] * frac;
            m += 1;
            k += 1.0;
        }
        if m > 0 { acc / m as f64 } else { 0.0 }
    };
    let mut best = (coarse_period, 0.0f64, f64::MIN);
    let steps = 40i32;
    for e in -steps..=steps {
        let period = coarse_period * (1.0 + e as f64 * 0.0005);
        let p_int = period.ceil() as usize;
        for phi in 0..p_int {
            let v = comb_at(period, phi as f64);
            if v > best.2 {
                best = (period, phi as f64, v);
            }
        }
    }
    // Refine period and phase on a finer lattice around the best cell.
    for _ in 0..2 {
        let (p0, f0, _) = best;
        for ep in -10..=10 {
            let period = p0 * (1.0 + ep as f64 * 0.00005);
            for fp in -10..=10 {
                let phi = f0 + fp as f64 * 0.1;
                if phi < 0.0 {
                    continue;
                }
                let v = comb_at(period, phi);
                if v > best.2 {
                    best = (period, phi, v);
                }
            }
        }
    }
    let (period, phi, peak_comb) = best;
    let bpm = 60.0 * frame_rate / period;
    let p_int = period.ceil() as usize;
    let comb_mean = (0..p_int).map(|p| comb_at(period, p as f64)).sum::<f64>() / p_int as f64;
    let phase_confidence = if comb_mean > 0.0 { peak_comb / comb_mean } else { 0.0 };
    // Frame f covers samples [f*HOP, f*HOP+WINDOW). Flux is largest at the
    // first frame whose window contains the transient, so the onset sits near
    // the window end minus one hop. ONSET_LAG_SAMPLES is the measured bias
    // against synthetic clicks (see tests).
    let onset_sample = phi * HOP as f64 + ONSET_LAG_SAMPLES;
    let mut offset_s = onset_sample / sample_rate as f64;
    let spb = 60.0 / bpm;
    while offset_s >= spb {
        offset_s -= spb;
    }
    while offset_s < 0.0 {
        offset_s += spb;
    }
    let beats = ((duration_s - offset_s) / spb).floor();
    Analysis {
        bpm,
        offset_s,
        tempo_confidence,
        phase_confidence,
        beats,
        bars: (beats / 4.0).floor(),
        ..empty
    }
}

/// Mean onset strength per bar on the detected grid, normalised so the
/// loudest bar is 1.0. Quiet bars (intros, breakdowns) read low; this is what
/// the automatic section map keys pattern intensity from.
pub fn bar_energy(a: &Analysis, bpm: f64, offset_s: f64, bars: usize) -> Vec<f64> {
    if a.onset.is_empty() || bpm <= 0.0 {
        return vec![0.0; bars];
    }
    let spb = 60.0 / bpm;
    let sr = a.sample_rate as f64;
    let mut out = Vec::with_capacity(bars);
    for b in 0..bars {
        let t0 = offset_s + b as f64 * 4.0 * spb;
        let t1 = t0 + 4.0 * spb;
        let f0 = ((t0 * sr - ONSET_LAG_SAMPLES) / HOP as f64).floor().max(0.0) as usize;
        let f1 = ((t1 * sr - ONSET_LAG_SAMPLES) / HOP as f64).floor().max(0.0) as usize;
        let f1 = f1.min(a.onset.len());
        if f0 >= f1 {
            out.push(0.0);
            continue;
        }
        let sum: f64 = a.onset[f0..f1].iter().map(|v| *v as f64).sum();
        out.push(sum / (f1 - f0) as f64);
    }
    let peak = out.iter().copied().fold(0.0, f64::max);
    if peak > 0.0 {
        for v in out.iter_mut() {
            *v /= peak;
        }
    }
    out
}

/// Read a WAV file (any channel count, 16/24/32-bit int or float) as mono f32.
pub fn read_wav_mono(path: &str) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("{path}: {e}"))?;
    let spec = reader.spec();
    let ch = spec.channels.max(1) as usize;
    let scale = match spec.sample_format {
        hound::SampleFormat::Float => 1.0,
        hound::SampleFormat::Int => ((1u64 << (spec.bits_per_sample - 1)) as f32).max(1.0),
    };
    let mut mono = Vec::new();
    let mut acc = 0.0f32;
    let mut k = 0usize;
    let mut push = |v: f32| {
        acc += v;
        k += 1;
        if k == ch {
            mono.push(acc / ch as f32);
            acc = 0.0;
            k = 0;
        }
    };
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                push(s.map_err(|e| e.to_string())?);
            }
        }
        hound::SampleFormat::Int => {
            for s in reader.samples::<i32>() {
                push(s.map_err(|e| e.to_string())? as f32 / scale);
            }
        }
    }
    Ok((mono, spec.sample_rate))
}

/// Deterministic synthetic material for tests and demos: kick + hat + bass
/// pulse on a beat grid at `bpm` starting at `offset_s`, with a noise floor.
pub fn synth_track(bpm: f64, offset_s: f64, seconds: f64, sample_rate: u32, seed: u64) -> Vec<f32> {
    synth_track_sections(bpm, offset_s, seconds, sample_rate, seed, 0)
}

/// Like `synth_track`, with the first `quiet_bars` bars reduced to the bass
/// pulse alone at low gain: an intro the section map must read as quiet.
pub fn synth_track_sections(bpm: f64, offset_s: f64, seconds: f64, sample_rate: u32, seed: u64, quiet_bars: usize) -> Vec<f32> {
    let n = (seconds * sample_rate as f64) as usize;
    let mut out = vec![0.0f32; n];
    let sr = sample_rate as f64;
    let spb = 60.0 / bpm;
    let mut rng = crate::sim::Rng::new(seed);
    for v in out.iter_mut() {
        *v = (rng.unit() as f32 - 0.5) * 0.02;
    }
    let mut beat = 0usize;
    loop {
        let t0 = offset_s + beat as f64 * spb;
        if t0 >= seconds {
            break;
        }
        let start = (t0 * sr) as usize;
        let quiet = beat / 4 < quiet_bars;
        // kick on every beat, accent on bar start
        let kick_len = (0.12 * sr) as usize;
        let gain = if quiet { 0.0 } else if beat % 4 == 0 { 0.9 } else { 0.7 };
        for i in 0..kick_len {
            let idx = start + i;
            if idx >= n {
                break;
            }
            let t = i as f64 / sr;
            let env = (1.0 - i as f64 / kick_len as f64).powi(2);
            let f = 55.0 + 90.0 * (-t * 40.0).exp();
            out[idx] += (std::f64::consts::TAU * f * t).sin() as f32 * (env * gain) as f32;
        }
        // hat on the off-beat eighth
        let hat_start = ((t0 + spb * 0.5) * sr) as usize;
        let hat_len = (0.03 * sr) as usize;
        for i in 0..hat_len {
            let idx = hat_start + i;
            if idx >= n {
                break;
            }
            let env = 1.0 - i as f32 / hat_len as f32;
            out[idx] += (rng.unit() as f32 - 0.5) * (if quiet { 0.0 } else { 0.5 }) * env;
        }
        // sustained bass note for the whole beat (masks the grid, like a real mix)
        let bass_len = (spb * sr * 0.9) as usize;
        for i in 0..bass_len {
            let idx = start + i;
            if idx >= n {
                break;
            }
            let t = i as f64 / sr;
            out[idx] += (std::f64::consts::TAU * 110.0 * t).sin() as f32 * (if quiet { 0.08 } else { 0.15 });
        }
        beat += 1;
    }
    for v in out.iter_mut() {
        *v = v.clamp(-1.0, 1.0);
    }
    out
}
