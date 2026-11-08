//! dbsong: render the produced tracks' stems for the engine.
//!   dbsong --out ../godot/audio              every track, four stems each
//!   dbsong --out dir --track skyline         one track
//!   dbsong --check                           render in memory and verify the analyser reads each grid back
//!   dbsong --list                            the plans

use downbeat_core::analysis::analyze;
use downbeat_core::song::{mix, render_track, tracks, write_stems, Hats, Track, SAMPLE_RATE};

fn print_plan(t: &Track) {
    println!("{}  {:.0} BPM  {} bars  {:.1} s", t.title, t.bpm, t.total_bars(), t.seconds());
    let mut bar = 0;
    for s in &t.plan {
        println!(
            "  {:>3}  {:<7} {:>2} bars  {}{}{}{}{}{}{}{}",
            bar + 1,
            s.name,
            s.bars,
            if s.kick { "kick " } else { "" },
            if s.bass { "bass " } else { "" },
            if s.pad { "pad " } else { "" },
            match s.hats {
                Hats::None => "",
                Hats::Eighths => "hats8 ",
                Hats::Sixteenths => "hats16 ",
            },
            if s.clap { "clap " } else { "" },
            if s.keys { "keys " } else { "" },
            if s.lead { "lead " } else { "" },
            if s.snare { "snare" } else { "" }
        );
        bar += s.bars;
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut out = String::new();
    let mut only: Option<String> = None;
    let mut check = false;
    let mut list = false;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--out" => {
                i += 1;
                out = argv.get(i).cloned().unwrap_or_default();
            }
            "--track" => {
                i += 1;
                only = argv.get(i).cloned();
            }
            "--check" => check = true,
            "--list" => list = true,
            "-h" | "--help" => {
                println!("dbsong [--out DIR] [--track NAME] [--check] [--list]");
                return;
            }
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let selected: Vec<Track> = tracks().into_iter().filter(|t| only.as_deref().is_none_or(|n| n == t.name)).collect();
    if selected.is_empty() {
        eprintln!("no such track; tracks: {}", tracks().iter().map(|t| t.name).collect::<Vec<_>>().join(", "));
        std::process::exit(2);
    }
    if list || (out.is_empty() && !check) {
        for t in &selected {
            print_plan(t);
        }
        if out.is_empty() && !check {
            eprintln!("give --out DIR or --check");
        }
        return;
    }
    let mut ok = true;
    for t in &selected {
        print_plan(t);
        let t0 = std::time::Instant::now();
        if check {
            let stems = render_track(t);
            let a = analyze(&mix(&stems), SAMPLE_RATE);
            let spb = 60.0 / t.bpm;
            // 25 ms: a syncopated kick (Undertow's pickup into 3) pulls the phase estimate by ~20 ms.
            let grid_ok = (a.bpm - t.bpm).abs() < 0.2 && a.offset_s.min((spb - a.offset_s).abs()) < 0.025;
            println!(
                "  rendered in {:.1} s; analyser reads {:.2} BPM, beat 0 at {:.3} s, tempo conf {:.1}x phase conf {:.1}x   grid check: {}",
                t0.elapsed().as_secs_f64(),
                a.bpm,
                a.offset_s,
                a.tempo_confidence,
                a.phase_confidence,
                if grid_ok { "PASS" } else { "FAIL" }
            );
            ok &= grid_ok;
        } else {
            match write_stems(&out, t) {
                Ok(paths) => {
                    for p in paths {
                        println!("  wrote {p}");
                    }
                    println!("  done in {:.1} s", t0.elapsed().as_secs_f64());
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
    }
    std::process::exit(if ok { 0 } else { 1 });
}
