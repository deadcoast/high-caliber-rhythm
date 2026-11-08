//! dbmap: analyse a song (WAV) for tempo and beat-0 offset, then write a
//! level JSON whose patterns fill the song's bars. One command, whole job.
//!
//!   dbmap song.wav                          analyse (wav/flac/ogg/mp3/m4a), print, write levels/<song>.json next to it
//!   dbmap song.wav --set flick --out ../godot/levels --audio res://audio/song.wav
//!   dbmap --synth 140 0.42 30               analyse a synthetic track (self-test material)
//!   dbmap song.wav --bpm 128 --offset 0.35  override detection (keep the analysis printout)
//!   dbmap song.wav --auto                   pick each bar's pattern from the song's own energy

use downbeat_core::analysis::{analyze, auto_sections, bar_energy, read_audio_mono, synth_track_sections, Analysis, HOP, ONSET_LAG_SAMPLES};
use downbeat_core::chart::{compile, Level, Section};
use downbeat_core::patterns::builtin_lib;
use downbeat_core::rating::rate;
use std::io::Write;

const SETS: &[(&str, &[&str])] = &[
    ("warmup", &["flick_lr", "flick_quad", "switch_pair", "micro_walk", "track_sweep", "flick_8ths", "micro_zigzag", "track_arc"]),
    ("flick", &["flick_lr", "flick_quad", "flick_8ths", "flick_lr", "flick_8ths"]),
    ("rush", &["flick_quad", "rush_fill", "flick_8ths", "rush_fill"]),
    ("switch", &["switch_pair", "switch_pair", "flick_quad", "switch_pair"]),
    ("track", &["track_sweep", "track_arc", "track_sweep", "rest"]),
    ("micro", &["micro_walk", "micro_zigzag", "micro_walk", "flick_lr"]),
];

fn usage() -> ! {
    println!("dbmap <song.wav> [--set warmup|flick|rush|switch|track|micro | --auto] [--name NAME] [--out DIR] [--audio RES_PATH] [--bpm N] [--offset S]");
    println!("dbmap --synth BPM OFFSET SECONDS [QUIET_BARS] [same options]");
    std::process::exit(2)
}

fn bar(frac: f64, width: usize) -> String {
    let n = (frac.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("{}{}", "#".repeat(n), ".".repeat(width - n))
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
    }
    let mut wav: Option<String> = None;
    let mut synth: Option<(f64, f64, f64, usize)> = None;
    let mut auto = false;
    let mut set = "warmup".to_string();
    let mut name: Option<String> = None;
    let mut out_dir: Option<String> = None;
    let mut audio: Option<String> = None;
    let mut bpm_override: Option<f64> = None;
    let mut offset_override: Option<f64> = None;
    let mut i = 0;
    let next = |i: &mut usize| -> String {
        *i += 1;
        argv.get(*i).cloned().unwrap_or_else(|| usage())
    };
    while i < argv.len() {
        match argv[i].as_str() {
            "--synth" => {
                let b: f64 = next(&mut i).parse().unwrap_or_else(|_| usage());
                let o: f64 = next(&mut i).parse().unwrap_or_else(|_| usage());
                let s: f64 = next(&mut i).parse().unwrap_or_else(|_| usage());
                let quiet = argv.get(i + 1).and_then(|q| q.parse::<usize>().ok()).map(|q| {
                    i += 1;
                    q
                }).unwrap_or(0);
                synth = Some((b, o, s, quiet));
            }
            "--auto" => auto = true,
            "--set" => set = next(&mut i),
            "--name" => name = Some(next(&mut i)),
            "--out" => out_dir = Some(next(&mut i)),
            "--audio" => audio = Some(next(&mut i)),
            "--bpm" => bpm_override = Some(next(&mut i).parse().unwrap_or_else(|_| usage())),
            "--offset" => offset_override = Some(next(&mut i).parse().unwrap_or_else(|_| usage())),
            "-h" | "--help" => usage(),
            other if other.starts_with("--") => usage(),
            other => wav = Some(other.to_string()),
        }
        i += 1;
    }
    let patterns = SETS.iter().find(|(n, _)| *n == set).map(|(_, p)| *p).unwrap_or_else(|| {
        eprintln!("unknown set '{set}'");
        usage()
    });

    print!("analysing ... ");
    std::io::stdout().flush().ok();
    let t0 = std::time::Instant::now();
    let (samples, sr, source) = match (&wav, synth) {
        (Some(path), _) => {
            let (s, sr) = read_audio_mono(path).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1)
            });
            (s, sr, path.clone())
        }
        (None, Some((b, o, secs, quiet))) => (
            synth_track_sections(b, o, secs, 44_100, 3, quiet),
            44_100,
            format!("synth {b} BPM offset {o} s{}", if quiet > 0 { format!(", {quiet} quiet bars") } else { String::new() }),
        ),
        _ => usage(),
    };
    let a: Analysis = analyze(&samples, sr);
    println!("{:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);

    let bpm = bpm_override.unwrap_or(a.bpm);
    let offset = offset_override.unwrap_or(a.offset_s);
    let spb = 60.0 / bpm;
    let beats = ((a.duration_s - offset) / spb).floor();
    let bars = (beats / 4.0).floor();

    const W: usize = 66;
    let rule = "-".repeat(W + 2);
    let row = |s: String| println!("| {:<W$} |", s);
    println!("+{rule}+");
    row(format!("DBMAP  {}", source));
    println!("+{rule}+");
    row(format!("length   {:.1} s   {} Hz   {} onset frames at {:.1}/s", a.duration_s, a.sample_rate, a.onset.len(), a.frame_rate));
    row(format!("tempo    {:.2} BPM   confidence {}  {:.1}x", a.bpm, bar((a.tempo_confidence - 1.0) / 6.0, 20), a.tempo_confidence));
    row(format!("beat 0   {:.4} s      confidence {}  {:.1}x", a.offset_s, bar((a.phase_confidence - 1.0) / 3.0, 20), a.phase_confidence));
    if bpm_override.is_some() || offset_override.is_some() {
        row(format!("using    {:.2} BPM   beat 0 {:.4} s   (override)", bpm, offset));
    }
    row(format!("grid     {} beats   {} bars   {:.3} s per beat", beats, bars, spb));
    row(String::new());
    // Onset curve, 8 bars from beat 0, one char per sixteenth: shows whether the grid sits on the hits.
    let mut line = String::new();
    let peak = a.onset.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    for k in 0..64 {
        let t = offset + k as f64 * spb / 4.0;
        let centre = ((t * a.sample_rate as f64 - ONSET_LAG_SAMPLES) / HOP as f64).round().max(0.0) as usize;
        let v = (centre.saturating_sub(1)..=centre + 1)
            .filter_map(|i| a.onset.get(i).copied())
            .fold(0.0f32, f32::max)
            / peak;
        line.push(match v {
            v if v > 0.6 => '#',
            v if v > 0.3 => '+',
            v if v > 0.12 => '-',
            _ => '.',
        });
    }
    row("onsets on the sixteenth grid, bars 1-4 from beat 0 (| = beat):".into());
    let mut marked = String::new();
    for (k, ch) in line.chars().enumerate() {
        if k % 4 == 0 {
            marked.push('|');
        }
        marked.push(ch);
    }
    row(marked[..marked.len().min(W)].to_string());
    let energy = bar_energy(&a, bpm, offset, bars.max(0.0) as usize);
    if !energy.is_empty() {
        row(String::new());
        row("energy per bar (one column each, 8 levels):".into());
        let glyphs = [' ', '.', ':', '-', '=', '+', '*', '#'];
        let mut e_line = String::new();
        for e in energy.iter().take(W) {
            let g = ((e * 7.0).round() as usize).min(7);
            e_line.push(glyphs[g]);
        }
        row(e_line);
    }
    println!("+{rule}+");

    // --- level
    let lead_in = 8.0;
    let usable_bars = (bars - 2.0 - 1.0).max(1.0) as usize;
    let mut sections: Vec<Section> = Vec::new();
    if auto {
        sections = auto_sections(&energy, bars.max(0.0) as usize);
    } else {
        let mut k = 0usize;
        let mut filled = 0usize;
        while filled < usable_bars {
            let p = patterns[k % patterns.len()];
            let reps = 2usize.min(usable_bars - filled);
            sections.push(Section { pattern: p.to_string(), repeats: reps as u32 });
            filled += reps;
            k += 1;
        }
    }
    let stem = wav
        .as_ref()
        .map(|p| std::path::Path::new(p).file_stem().and_then(|s| s.to_str()).unwrap_or("song").to_string())
        .unwrap_or_else(|| "synth".to_string());
    let level_name = name.unwrap_or_else(|| format!("{}_{}", stem, bpm.round() as i64));
    let level = Level {
        name: level_name.clone(),
        bpm,
        lead_in,
        sections,
        audio: audio.unwrap_or_default(),
        song_offset_s: offset,
        ..Level::default()
    };
    let json = level.to_json_pretty();
    if let Ok(chart) = compile(&level, &builtin_lib()) {
        let r = rate(&chart);
        println!("rating   {:.2} stars   aim {:.0} deg/s   density {:.2}/s   tracking {:.1}   fingerprint {:016x}", r.stars, r.aim_speed, r.density, r.tracking, chart.fingerprint());
    }
    let dir = out_dir.unwrap_or_else(|| {
        wav.as_ref()
            .and_then(|p| std::path::Path::new(p).parent().map(|d| d.to_string_lossy().to_string()))
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| ".".to_string())
    });
    let path = format!("{}/{}.json", dir.trim_end_matches('/'), level_name);
    match std::fs::write(&path, &json) {
        Ok(()) => println!("level    {}   {} sections   {}", path, level.sections.len(), if auto { "auto (energy-mapped)".to_string() } else { format!("set '{set}'") }),
        Err(e) => {
            eprintln!("could not write {path}: {e}");
            println!("{json}");
        }
    }
}
