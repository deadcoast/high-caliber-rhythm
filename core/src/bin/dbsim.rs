//! dbsim: play the warmup chart with a bot at one or several BPMs and print
//! the run report. One command, whole job.
//!
//!   dbsim                      human bot, 128 BPM
//!   dbsim --bpm 174            one tempo
//!   dbsim --sweep              90 / 128 / 174 side by side
//!   dbsim --bot perfect        the clock and judge with zero human error
//!   dbsim --jitter 35 --bias 20 --seed 3
//!   dbsim --level-json 128 > ../godot/levels/warmup_128.json
//!   dbsim --level ../godot/levels/night_market_132.json   play a level file

use downbeat_core::chart::Level;
use downbeat_core::patterns::{fill_level, street_hard_level, street_level, warmup_level};
use downbeat_core::score::Report;
use downbeat_core::sim::{run_level, Bot, SimConfig, SimResult};
use std::io::Write;

struct Args {
    bpms: Vec<f64>,
    bot: Bot,
    seed: u64,
    no_audio: bool,
    fill: bool,
    street: bool,
    hard: bool,
    level: Option<Level>,
}

fn parse() -> Args {
    let mut bpms = vec![128.0];
    let mut bot = Bot::human(22.0);
    let mut seed = 7u64;
    let mut no_audio = false;
    let mut fill = false;
    let mut street = false;
    let mut hard = false;
    let mut level: Option<Level> = None;
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    let take = |i: &mut usize| -> String {
        *i += 1;
        argv.get(*i).cloned().unwrap_or_else(|| {
            eprintln!("missing value for {}", argv[*i - 1]);
            std::process::exit(2)
        })
    };
    while i < argv.len() {
        match argv[i].as_str() {
            "--bpm" => bpms = vec![take(&mut i).parse().expect("bpm number")],
            "--sweep" => bpms = vec![90.0, 128.0, 174.0],
            "--bot" => {
                bot = match take(&mut i).as_str() {
                    "perfect" => Bot::perfect(),
                    "human" => Bot::human(22.0),
                    other => {
                        eprintln!("unknown bot '{other}' (perfect | human)");
                        std::process::exit(2)
                    }
                }
            }
            "--jitter" => bot.jitter_ms = take(&mut i).parse().expect("jitter ms"),
            "--bias" => bot.bias_ms = take(&mut i).parse().expect("bias ms"),
            "--hit" => bot.hit_prob = take(&mut i).parse().expect("hit prob"),
            "--seed" => seed = take(&mut i).parse().expect("seed"),
            "--no-audio" => no_audio = true,
            "--fill" => fill = true,
            "--street" => street = true,
            "--hard" => hard = true,
            "--level" => {
                let path = take(&mut i);
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                    eprintln!("{path}: {e}");
                    std::process::exit(2)
                });
                let l = Level::from_json(&text).unwrap_or_else(|e| {
                    eprintln!("{path}: {e}");
                    std::process::exit(2)
                });
                bpms = vec![l.bpm];
                level = Some(l);
            }
            "--level-json" => {
                // Print the warmup level as JSON (levels/*.json authoring reference) and exit.
                let bpm: f64 = take(&mut i).parse().expect("bpm number");
                let level = if argv.iter().any(|a| a == "--hard") {
                    street_hard_level(bpm)
                } else if argv.iter().any(|a| a == "--street") {
                    street_level(bpm)
                } else if argv.iter().any(|a| a == "--fill") {
                    fill_level(bpm)
                } else {
                    warmup_level(bpm)
                };
                println!("{}", level.to_json_pretty());
                std::process::exit(0)
            }
            "-h" | "--help" => {
                println!("dbsim [--bpm N | --sweep] [--fill | --street | --hard | --level FILE] [--bot perfect|human] [--jitter MS] [--bias MS] [--hit P] [--seed N] [--no-audio] [--level-json BPM [--fill]]");
                std::process::exit(0)
            }
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2)
            }
        }
        i += 1;
    }
    Args { bpms, bot, seed, no_audio, fill, street, hard, level }
}

fn bar(frac: f64, width: usize) -> String {
    let n = (frac.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("{}{}", "#".repeat(n), ".".repeat(width - n))
}

fn pct(x: f64) -> String {
    format!("{:5.1}%", x * 100.0)
}

fn print_run(bpm: f64, r: &SimResult) {
    const W: usize = 74;
    let rule = "-".repeat(W + 2);
    let row = |s: String| println!("| {:<W$} |", s);
    let rep: &Report = &r.report;
    println!("+{rule}+");
    row(format!("DOWNBEAT  {} @ {bpm:.0} BPM   {}   grade {}   score {}", r.name, rep.mods, rep.grade.label(), rep.score));
    row(format!("          {:.0} pp   {:.2} stars{}", rep.pp, rep.stars, if rep.pfc { "   PFC" } else if rep.fc { "   FC" } else { "" }));
    println!("+{rule}+");
    let judged = (rep.hits + rep.expired).max(1) as f64;
    let rows = [
        ("PERFECT", rep.perfect),
        ("GREAT", rep.great),
        ("GOOD", rep.good),
        ("EARLY", rep.early),
        ("LATE", rep.late),
        ("EXPIRED", rep.expired),
    ];
    for (name, n) in rows {
        let f = n as f64 / judged;
        row(format!("{:<8} {:>4}  {}  {}", name, n, bar(f, 50), pct(f)));
    }
    row(String::new());
    row(format!("aim      hits {}/{} shots   acc {}   precision {}   whiffs {}", rep.hits, rep.shots, pct(rep.aim_accuracy), pct(rep.precision), rep.whiffs));
    row(format!("timing   on-beat {}   quality {}   offset {:+.1} ms   sd {:.1} ms", pct(rep.beat_accuracy), pct(rep.timing_quality), rep.timing_mean_ms, rep.timing_std_ms));
    row(format!("track    on-target {}   combo max {}   damage {}   downs {}   hp {}", pct(rep.track_ratio), rep.max_combo, rep.damage_taken, rep.downs, rep.hp));
    if rep.phases_cleared + rep.phase_fails > 0 {
        row(format!("boss     phases cleared {}   phases lost {}", rep.phases_cleared, rep.phase_fails));
    }
    row(String::new());
    row(format!("{:<10} {:>5} {:>5} {:>8} {:>8} {:>10} {:>8}", "KIND", "tgts", "hits", "on-beat", "expired", "precision", "track"));
    for (k, s) in &rep.by_kind {
        let prec = if s.hits > 0 && s.track_total == 0 { format!("{:>9.0}%", 100.0 * s.precision_sum / s.hits as f64) } else { format!("{:>10}", "-") };
        let track = if s.track_total > 0 { format!("{:>7.0}%", 100.0 * s.track_on as f64 / s.track_total as f64) } else { format!("{:>8}", "-") };
        row(format!("{:<10} {:>5} {:>5} {:>8} {:>8} {} {}", k, s.targets, s.hits, s.on_beat, s.expired, prec, track));
    }
    row(String::new());
    row(format!("clock    {} frames   {} ticks   {} beats   {} bars   {} spawns", r.frames, r.ticks, r.beats, r.bars, r.spawns));
    row(format!("audio    drift rms {:.2} ms   max {:.2} ms   snaps {}   restarts {}   simulated {:.1} s", r.rms_drift_ms, r.max_abs_drift_ms, r.snaps, r.restarts, r.sim_time_s));
    println!("+{rule}+");
}

fn main() {
    let args = parse();
    let cfg = SimConfig { seed: args.seed, audio_clock: !args.no_audio, ..SimConfig::default() };
    let b = args.bot;
    println!(
        "bot  bias {:+.0} ms  jitter {:.0} ms  hit {:.0}%  spread {:.2}  skip {:.0}%  track-on {:.0}%  seed {}",
        b.bias_ms, b.jitter_ms, b.hit_prob * 100.0, b.spread, b.skip_prob * 100.0, b.track_on_prob * 100.0, args.seed
    );
    let mut results = Vec::new();
    for &bpm in &args.bpms {
        print!("running {bpm:.0} BPM ... ");
        std::io::stdout().flush().ok();
        let t0 = std::time::Instant::now();
        let level = match &args.level {
            Some(l) => {
                let mut l = l.clone();
                l.bpm = bpm;
                l
            }
            None if args.hard => street_hard_level(bpm),
            None if args.street => street_level(bpm),
            None if args.fill => fill_level(bpm),
            None => warmup_level(bpm),
        };
        let r = run_level(&level, args.bot, cfg);
        println!("{:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
        results.push((bpm, r));
    }
    println!();
    for (bpm, r) in &results {
        print_run(*bpm, r);
        println!();
    }
    if results.len() > 1 {
        println!("{:>6} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}", "BPM", "grade", "score", "on-beat", "quality", "sd ms", "expired");
        for (bpm, r) in &results {
            let rep = &r.report;
            println!("{:>6.0} {:>6} {:>8} {:>8} {:>8} {:>8.1} {:>8}", bpm, rep.grade.label(), rep.score, pct(rep.beat_accuracy), pct(rep.timing_quality), rep.timing_std_ms, rep.expired);
        }
    }
}
