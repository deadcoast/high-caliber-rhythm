//! Produced tracks, rendered by the core: the content pipeline needs songs
//! with stems, measured grids and hand charts before commissioned ones
//! exist. Act one has three, each a section plan over a progression with a
//! motif of its own, rendered deterministically to four stems that the
//! engine opens by combo tier:
//!   0 base   kick, bass, pad          (always on)
//!   1 perc   hats, clap               (combo 10)
//!   2 keys   chord stabs              (combo 25)
//!   3 lead   melody, snare            (combo 50)
//! Beat 0 is the first sample. SCAFFOLD AUDIO (ASSETS.register.md [AUD-03]):
//! commissioned tracks replace the stems; the plans, the levels and the
//! pipeline stay.
//!   NIGHT MARKET  132 BPM  A minor   88 bars   street, atrium, platform
//!   UNDERTOW      150 BPM  D minor   88 bars   underpass, lobby (half-time kick)
//!   SKYLINE       168 BPM  E minor  112 bars   rooftop, helipad; three boss phases

use std::f64::consts::{PI, TAU};

pub const SAMPLE_RATE: u32 = 44100;
pub const STEMS: [&str; 4] = ["base", "perc", "keys", "lead"];

/// One section of the arrangement.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub name: &'static str,
    pub bars: u32,
    pub kick: bool,
    pub bass: bool,
    pub pad: bool,
    pub hats: Hats,
    pub clap: bool,
    pub keys: bool,
    pub lead: bool,
    pub snare: bool,
    /// Output gain of the whole section (fade in/out).
    pub level: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hats {
    None,
    Eighths,
    Sixteenths,
}

/// One track: the plan, the harmony, the rhythms.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: &'static str,
    pub title: &'static str,
    pub bpm: f64,
    /// Root MIDI notes of the progression, one bar each.
    pub progression: [i32; 4],
    /// Which roots take a minor triad (the rest are major).
    pub minor: [bool; 4],
    /// Tonic MIDI note the motifs are written from.
    pub tonic: i32,
    /// Lead motifs over two bars in eighths, as semitones over the tonic
    /// (i32::MIN = rest); the answer varies the end.
    pub motif_a: [i32; 16],
    pub motif_b: [i32; 16],
    /// Kick, keys stab and bass patterns per bar in sixteenths
    /// (bass: 0 rest, 1 root, 2 octave).
    pub kick: [u8; 16],
    pub stab: [u8; 16],
    pub bass: [u8; 16],
    pub plan: Vec<Section>,
}

const R: i32 = i32::MIN;

fn sec(name: &'static str, bars: u32, kick: bool, bass: bool, pad: bool, hats: Hats, clap: bool, keys: bool, lead: bool, snare: bool, level: f32) -> Section {
    Section { name, bars, kick, bass, pad, hats, clap, keys, lead, snare, level }
}

/// Act one's three tracks.
pub fn tracks() -> Vec<Track> {
    vec![
        Track {
            name: "night_market",
            title: "NIGHT MARKET",
            bpm: 132.0,
            progression: [57, 53, 48, 55], // A3 F3 C3 G3: i VI III VII
            minor: [true, false, false, false],
            tonic: 69,
            motif_a: [12, R, 15, R, 19, 17, R, 15, 12, R, 10, 12, R, 7, R, 10],
            motif_b: [12, R, 15, R, 19, 17, R, 15, 22, R, 19, 17, 15, R, 12, R],
            kick: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
            stab: [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0],
            bass: [1, 0, 1, 0, 2, 0, 1, 0, 1, 0, 2, 0, 1, 1, 0, 2],
            plan: vec![
                sec("intro", 8, false, true, true, Hats::None, false, false, false, false, 0.9),
                sec("build", 8, true, true, true, Hats::Eighths, false, true, false, false, 1.0),
                sec("verse", 16, true, true, true, Hats::Eighths, true, true, false, true, 1.0),
                sec("chorus", 16, true, true, true, Hats::Sixteenths, true, true, true, true, 1.0),
                sec("break", 8, false, false, true, Hats::None, false, false, true, false, 0.85),
                sec("build2", 8, true, true, true, Hats::Sixteenths, true, true, false, false, 1.0),
                sec("drop", 16, true, true, true, Hats::Sixteenths, true, true, true, true, 1.0),
                sec("outro", 8, false, true, true, Hats::Eighths, false, true, false, false, 0.8),
            ],
        },
        Track {
            name: "undertow",
            title: "UNDERTOW",
            bpm: 150.0,
            progression: [50, 46, 41, 48], // D3 Bb2 F2 C3: i VI III VII in D minor
            minor: [true, false, false, false],
            tonic: 62,
            // Half-time: the kick on 1 and a pickup into 3; the melody
            // leans on the fifth and the flat seventh, longer notes.
            motif_a: [7, R, R, 10, 12, R, 10, R, 7, R, 5, R, 3, R, R, 5],
            motif_b: [7, R, R, 10, 12, R, 15, R, 14, R, 12, R, 10, R, 7, R],
            kick: [1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            stab: [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
            bass: [1, 0, 1, 0, 2, 0, 1, 0, 1, 0, 1, 0, 2, 0, 1, 0],
            plan: vec![
                sec("intro", 8, false, false, true, Hats::None, false, true, false, false, 0.85),
                sec("verse", 16, true, true, true, Hats::Eighths, true, true, false, true, 1.0),
                sec("build", 8, true, true, true, Hats::Sixteenths, true, true, true, false, 1.0),
                sec("chorus", 16, true, true, true, Hats::Sixteenths, true, true, true, true, 1.0),
                sec("break", 8, false, true, true, Hats::None, false, false, true, false, 0.85),
                sec("verse2", 8, true, true, true, Hats::Eighths, true, true, false, true, 1.0),
                sec("drop", 16, true, true, true, Hats::Sixteenths, true, true, true, true, 1.0),
                sec("outro", 8, false, false, true, Hats::Eighths, false, true, false, false, 0.8),
            ],
        },
        Track {
            name: "skyline",
            title: "SKYLINE",
            bpm: 168.0,
            progression: [52, 48, 43, 50], // E3 C3 G2 D3: i VI III VII in E minor
            minor: [true, false, false, false],
            tonic: 64,
            // Fast: eighth-note runs up the pentatonic and a held top note.
            motif_a: [12, 15, 17, 19, 22, R, 19, R, 17, 15, 12, R, 10, R, 12, R],
            motif_b: [12, 15, 17, 19, 22, R, 24, R, 22, R, 19, 17, 15, R, 12, R],
            kick: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
            stab: [1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0],
            bass: [1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 2, 0, 1, 0, 2, 2],
            plan: vec![
                sec("intro", 8, false, true, true, Hats::Eighths, false, false, false, false, 0.9),
                sec("build", 8, true, true, true, Hats::Eighths, false, true, false, false, 1.0),
                sec("verse", 16, true, true, true, Hats::Sixteenths, true, true, false, true, 1.0),
                sec("chorus", 16, true, true, true, Hats::Sixteenths, true, true, true, true, 1.0),
                sec("break", 8, false, false, true, Hats::None, false, false, true, false, 0.85),
                sec("boss1", 16, true, true, true, Hats::Sixteenths, true, true, false, true, 1.0),
                sec("boss2", 16, true, true, true, Hats::Sixteenths, false, false, true, true, 1.0),
                sec("build3", 8, true, true, true, Hats::Sixteenths, true, true, false, false, 1.0),
                sec("boss3", 16, true, true, true, Hats::Sixteenths, true, true, true, true, 1.0),
                sec("outro", 8, false, true, true, Hats::Eighths, false, true, false, false, 0.8),
            ],
        },
    ]
}

pub fn track(name: &str) -> Option<Track> {
    tracks().into_iter().find(|t| t.name == name)
}

/// Night Market's plan (the first track), kept for the engine's doc and the tests.
pub fn plan() -> Vec<Section> {
    tracks().remove(0).plan
}

pub fn total_bars() -> u32 {
    plan().iter().map(|s| s.bars).sum()
}

impl Track {
    pub fn total_bars(&self) -> u32 {
        self.plan.iter().map(|s| s.bars).sum()
    }

    pub fn seconds(&self) -> f64 {
        self.total_bars() as f64 * 4.0 * 60.0 / self.bpm
    }

    fn chord(&self, bar_in_prog: usize) -> [i32; 3] {
        if self.minor[bar_in_prog] {
            [0, 3, 7]
        } else {
            [0, 4, 7]
        }
    }
}

fn midi(n: i32) -> f64 {
    440.0 * 2f64.powf((n as f64 - 69.0) / 12.0)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) as f32 * 2.0 - 1.0
    }
}

/// Simple state-variable low-pass, 2-pole.
struct Svf {
    low: f32,
    band: f32,
}
impl Svf {
    fn new() -> Self {
        Self { low: 0.0, band: 0.0 }
    }
    fn run(&mut self, x: f32, cutoff_hz: f32, q: f32) -> f32 {
        let f = (2.0 * (PI as f32 * cutoff_hz.min(18000.0) / SAMPLE_RATE as f32).sin()).min(1.5);
        let high = x - self.low - q * self.band;
        self.band += f * high;
        self.low += f * self.band;
        self.low
    }
}

/// PolyBLEP saw: band-limited enough for a lead or a bass.
fn saw(phase: f64, dt: f64) -> f32 {
    let mut v = 2.0 * phase - 1.0;
    if phase < dt {
        let t = phase / dt;
        v -= t + t - t * t - 1.0;
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        v -= t * t + t + t + 1.0;
    }
    v as f32
}

fn env(t: f64, a: f64, d: f64, s: f64, r: f64, dur: f64) -> f32 {
    let g = if t < a {
        t / a
    } else if t < a + d {
        1.0 - (1.0 - s) * (t - a) / d
    } else {
        s
    };
    let rel = if t > dur { (1.0 - (t - dur) / r).max(0.0) } else { 1.0 };
    (g * rel) as f32
}

/// A stereo-less (mono) render buffer with note helpers.
struct Buf {
    data: Vec<f32>,
}

impl Buf {
    fn new(frames: usize) -> Self {
        Self { data: vec![0.0; frames] }
    }

    fn add(&mut self, at: usize, v: f32) {
        if let Some(x) = self.data.get_mut(at) {
            *x += v;
        }
    }

    /// Detuned two-saw voice through a low-pass with a pluck envelope.
    fn saw_note(&mut self, start_s: f64, dur_s: f64, freq: f64, gain: f32, detune: f64, cutoff: f32, cut_env: f32, attack: f64, release: f64) {
        let sr = SAMPLE_RATE as f64;
        let start = (start_s * sr) as usize;
        let n = ((dur_s + release) * sr) as usize;
        let (mut p1, mut p2) = (0.0f64, 0.37f64);
        let dt1 = freq / sr;
        let dt2 = freq * (1.0 + detune) / sr;
        let mut f = Svf::new();
        for i in 0..n {
            let t = i as f64 / sr;
            let e = env(t, attack, 0.08, 0.7, release, dur_s);
            let x = 0.5 * (saw(p1, dt1) + saw(p2, dt2));
            let cut = cutoff + cut_env * e;
            let y = f.run(x, cut, 0.6);
            self.add(start + i, y * e * gain);
            p1 = (p1 + dt1) % 1.0;
            p2 = (p2 + dt2) % 1.0;
        }
    }

    fn sine_sweep(&mut self, start_s: f64, dur_s: f64, f0: f64, f1: f64, gain: f32, curve: f64) {
        let sr = SAMPLE_RATE as f64;
        let start = (start_s * sr) as usize;
        let n = (dur_s * sr) as usize;
        let mut phase = 0.0f64;
        for i in 0..n {
            let u = i as f64 / n as f64;
            let f = f1 + (f0 - f1) * (1.0 - u).powf(curve);
            phase += TAU * f / sr;
            let e = (1.0 - u).powf(1.6) as f32;
            self.add(start + i, (phase.sin() as f32) * e * gain);
        }
    }

    fn noise(&mut self, start_s: f64, dur_s: f64, gain: f32, highpass_hz: f32, rng: &mut Rng, curve: f32) {
        let sr = SAMPLE_RATE as f64;
        let start = (start_s * sr) as usize;
        let n = (dur_s * sr) as usize;
        let mut f = Svf::new();
        for i in 0..n {
            let u = i as f32 / n as f32;
            let x = rng.next();
            // High-pass = input minus the low-pass.
            let y = x - f.run(x, highpass_hz, 0.8);
            self.add(start + i, y * (1.0 - u).powf(curve) * gain);
        }
    }

    fn normalise(&mut self, peak: f32) {
        let m = self.data.iter().fold(0.0f32, |a, &v| a.max(v.abs())).max(1e-6);
        let k = peak / m;
        for v in &mut self.data {
            *v *= k;
        }
    }

    fn apply_section_levels(&mut self, bpm: f64, plan: &[Section]) {
        let spb = 60.0 / bpm;
        let mut bar = 0u32;
        for s in plan {
            let a = (bar as f64 * 4.0 * spb * SAMPLE_RATE as f64) as usize;
            let b = ((bar + s.bars) as f64 * 4.0 * spb * SAMPLE_RATE as f64) as usize;
            for i in a..b.min(self.data.len()) {
                self.data[i] *= s.level;
            }
            bar += s.bars;
        }
        // Outro fade over the last four bars.
        let fade_start = ((bar as f64 - 4.0) * 4.0 * spb * SAMPLE_RATE as f64) as usize;
        let end = (bar as f64 * 4.0 * spb * SAMPLE_RATE as f64) as usize;
        for i in fade_start..self.data.len() {
            let u = ((i - fade_start) as f32 / (end - fade_start).max(1) as f32).min(1.0);
            self.data[i] *= 1.0 - u;
        }
    }
}

/// Lead delay: dotted-eighth feedback echo, the classic.
fn delay(buf: &mut Buf, bpm: f64, feedback: f32, mix: f32) {
    let d = (0.75 * 60.0 / bpm * SAMPLE_RATE as f64) as usize;
    let src = buf.data.clone();
    let mut line = vec![0.0f32; buf.data.len()];
    for i in 0..buf.data.len() {
        let fb = if i >= d { line[i - d] } else { 0.0 };
        line[i] = src[i] + fb * feedback;
        if i >= d {
            buf.data[i] += line[i - d] * mix;
        }
    }
}

/// Night Market at `bpm` (the first track); see `render_track`.
pub fn render(bpm: f64) -> Vec<(&'static str, Vec<f32>)> {
    let mut t = tracks().remove(0);
    t.bpm = bpm;
    render_track(&t)
}

/// Render a track's four stems. Returns (name, samples) in STEMS order,
/// all the same length, mono f32 in [-1, 1].
pub fn render_track(tr: &Track) -> Vec<(&'static str, Vec<f32>)> {
    let bpm = tr.bpm;
    let spb = 60.0 / bpm;
    let bars = tr.total_bars();
    let frames = ((bars as f64 * 4.0 * spb + 2.0) * SAMPLE_RATE as f64) as usize;
    let mut base = Buf::new(frames);
    let mut perc = Buf::new(frames);
    let mut keys = Buf::new(frames);
    let mut lead = Buf::new(frames);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ tr.bpm.to_bits());
    let sixteenth = spb / 4.0;
    let mut bar = 0u32;
    for s in &tr.plan {
        for b in 0..s.bars {
            let abs_bar = bar + b;
            let t0 = abs_bar as f64 * 4.0 * spb;
            let prog_i = (abs_bar % 4) as usize;
            let root = tr.progression[prog_i];
            let tones = tr.chord(prog_i);
            // Pad: chord held for the bar, soft.
            if s.pad {
                for (k, semi) in tones.iter().enumerate() {
                    let f = midi(root + 12 + semi);
                    base.saw_note(t0, 4.0 * spb - 0.05, f, 0.08, 0.004 + 0.002 * k as f64, 600.0, 300.0, 0.25, 0.3);
                }
            }
            // Bass: sixteenth pattern, root and octave, plucky.
            if s.bass {
                for (k, &v) in tr.bass.iter().enumerate() {
                    if v == 0 {
                        continue;
                    }
                    let n = root - 12 + if v == 2 { 12 } else { 0 };
                    base.saw_note(t0 + k as f64 * sixteenth, sixteenth * 0.9, midi(n), 0.42, 0.0, 120.0, 900.0, 0.004, 0.04);
                }
            }
            // Kick: the track's pattern; the last bar of a build gets a sixteenth roll.
            if s.kick {
                let roll = s.name.starts_with("build") && b == s.bars - 1;
                let steps: Vec<usize> = if roll { (0..16).collect() } else { tr.kick.iter().enumerate().filter(|(_, &v)| v != 0).map(|(i, _)| i).collect() };
                for k in steps {
                    let g = if roll { 0.55 + 0.45 * (k as f32 / 15.0) } else { 0.95 };
                    base.sine_sweep(t0 + k as f64 * sixteenth, 0.28, 160.0, 44.0, g, 2.5);
                    base.noise(t0 + k as f64 * sixteenth, 0.012, 0.3, 3000.0, &mut rng, 3.0);
                }
            }
            // Hats.
            match s.hats {
                Hats::None => {}
                Hats::Eighths => {
                    for k in (0..16).step_by(2) {
                        let accent = if k % 4 == 2 { 0.45 } else { 0.28 };
                        perc.noise(t0 + k as f64 * sixteenth, 0.045, accent, 7000.0, &mut rng, 2.0);
                    }
                }
                Hats::Sixteenths => {
                    for k in 0..16 {
                        let accent = match k % 4 {
                            0 => 0.22,
                            2 => 0.5,
                            _ => 0.3,
                        };
                        perc.noise(t0 + k as f64 * sixteenth, if k % 4 == 2 { 0.11 } else { 0.04 }, accent, 7000.0, &mut rng, 2.0);
                    }
                }
            }
            // Clap on 2 and 4: three bursts.
            if s.clap {
                for beat in [1, 3] {
                    let t = t0 + beat as f64 * spb;
                    for j in 0..3 {
                        perc.noise(t + j as f64 * 0.009, 0.14, 0.4, 1500.0, &mut rng, 2.5);
                    }
                }
            }
            // Keys: chord stabs, short, an octave up.
            if s.keys {
                for (k, &hit) in tr.stab.iter().enumerate() {
                    if hit == 0 {
                        continue;
                    }
                    for semi in tones {
                        keys.saw_note(t0 + k as f64 * sixteenth, sixteenth * 1.4, midi(root + 24 + semi), 0.10, 0.006, 900.0, 2200.0, 0.003, 0.06);
                    }
                }
            }
            // Snare on 2 and 4 (lead stem, top tier).
            if s.snare {
                for beat in [1, 3] {
                    let t = t0 + beat as f64 * spb;
                    lead.noise(t, 0.18, 0.5, 1800.0, &mut rng, 2.2);
                    lead.sine_sweep(t, 0.12, 190.0, 150.0, 0.5, 1.0);
                }
            }
            // Lead: the motif over two bars, answer on the second pair.
            if s.lead {
                let motif = if (abs_bar / 2) % 2 == 0 { &tr.motif_a } else { &tr.motif_b };
                let half = (abs_bar % 2) as usize * 8;
                for k in 0..8 {
                    let n = motif[half + k];
                    if n == R {
                        continue;
                    }
                    // Hold until the next note or rest.
                    let mut len = 1;
                    while half + k + len < 16 && motif[half + k + len] == R && len < 3 {
                        len += 1;
                    }
                    let gain = if s.name == "break" { 0.2 } else { 0.34 };
                    lead.saw_note(t0 + k as f64 * 2.0 * sixteenth, len as f64 * 2.0 * sixteenth * 0.85, midi(tr.tonic + n), gain, 0.008, 1400.0, 2600.0, 0.01, 0.12);
                }
            }
        }
        bar += s.bars;
    }
    delay(&mut lead, bpm, 0.38, 0.33);
    for b in [&mut base, &mut perc, &mut keys, &mut lead] {
        b.apply_section_levels(bpm, &tr.plan);
        b.normalise(0.89);
    }
    vec![("base", base.data), ("perc", perc.data), ("keys", keys.data), ("lead", lead.data)]
}

/// Write a track's stems as 16-bit mono WAVs: `<dir>/<name>_<stem>.wav`. Returns the paths.
pub fn write_stems(dir: &str, tr: &Track) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    let spec = hound::WavSpec { channels: 1, sample_rate: SAMPLE_RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut out = Vec::new();
    let name = tr.name;
    for (stem, samples) in render_track(tr) {
        let path = format!("{dir}/{name}_{stem}.wav");
        let mut w = hound::WavWriter::create(&path, spec).map_err(|e| format!("{path}: {e}"))?;
        for v in samples {
            w.write_sample((v.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
        out.push(path);
    }
    Ok(out)
}

/// Mono mix of the stems at equal gain, for the analyser check.
pub fn mix(stems: &[(&'static str, Vec<f32>)]) -> Vec<f32> {
    let n = stems.iter().map(|(_, s)| s.len()).max().unwrap_or(0);
    let mut out = vec![0.0f32; n];
    for (_, s) in stems {
        for (i, v) in s.iter().enumerate() {
            out[i] += v / stems.len() as f32;
        }
    }
    out
}
