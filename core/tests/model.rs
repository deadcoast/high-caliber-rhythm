use downbeat_core::chart::{compile, Kind, Level, Scaling, Section};
use downbeat_core::clock::{ClockConfig, ClockEvent, Conductor};
use downbeat_core::judge::{Hit, Judge, Shot, Timing, Verdict, Windows};
use downbeat_core::patterns::{builtin_lib, warmup_level};
use downbeat_core::score::{Grade, Scoring, Session};
use downbeat_core::sim::{run_level, Bot, SimConfig};
use downbeat_core::Game;

fn approx(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

// ---------------------------------------------------------------- clock

#[test]
fn beat_time_roundtrip_at_any_bpm() {
    for bpm in [60.0, 90.0, 128.0, 174.0, 200.0] {
        let c = Conductor::new(ClockConfig { bpm, song_offset_s: 0.35, ..Default::default() });
        for beat in [-4.0, 0.0, 1.0, 7.5, 63.0] {
            assert!(approx(c.time_to_beat(c.beat_to_time(beat)), beat, 1e-9));
        }
        assert!(approx(c.beat_to_time(1.0) - c.beat_to_time(0.0), 60.0 / bpm, 1e-12));
    }
}

#[test]
fn open_loop_emits_every_tick_beat_and_bar_in_order() {
    let mut c = Conductor::new(ClockConfig { bpm: 128.0, ..Default::default() });
    c.start(-0.5);
    let mut out = Vec::new();
    let dt = 1.0 / 240.0;
    let mut t = -0.5;
    while t < 10.0 {
        c.update(dt, None, &mut out);
        t += dt;
    }
    let ticks: Vec<i64> = out.iter().filter_map(|e| match e { ClockEvent::Tick { index, .. } => Some(*index), _ => None }).collect();
    let beats: Vec<i64> = out.iter().filter_map(|e| match e { ClockEvent::Beat { index, .. } => Some(*index), _ => None }).collect();
    let bars: Vec<i64> = out.iter().filter_map(|e| match e { ClockEvent::Bar { index, .. } => Some(*index), _ => None }).collect();
    // Ticks are consecutive integers, starting right after the pre-roll tick.
    for w in ticks.windows(2) {
        assert_eq!(w[1], w[0] + 1, "tick gap");
    }
    assert_eq!(*ticks.first().unwrap(), ((-0.5 * 128.0 / 60.0) * 4.0f64).floor() as i64 + 1);
    let expected_last_beat = (10.0 * 128.0 / 60.0f64).floor() as i64;
    assert_eq!(*beats.last().unwrap(), expected_last_beat);
    assert!(beats.contains(&0), "beat zero must fire");
    assert_eq!(bars, (0..=expected_last_beat / 4).collect::<Vec<_>>());
    // A Beat always immediately follows its Tick with sub 0.
    for (i, e) in out.iter().enumerate() {
        if let ClockEvent::Beat { index, in_bar } = e {
            assert_eq!(out[i - 1], ClockEvent::Tick { index: index * 4, beat: *index, sub: 0 });
            assert_eq!(*in_bar as i64, index.rem_euclid(4));
        }
    }
}

#[test]
fn audio_lock_pulls_to_jittery_clock_and_snaps_on_seek() {
    let mut c = Conductor::new(ClockConfig::default());
    c.start(0.0);
    let mut out = Vec::new();
    let dt = 1.0 / 144.0;
    let chunk = 512.0 / 44_100.0;
    let mut real: f64 = 0.0;
    let mut worst = 0.0f64;
    // Game clock runs 1% fast relative to audio to force correction.
    for i in 0..2000 {
        real += dt;
        let quantised = (real / chunk).floor() * chunk + (if i % 3 == 0 { 0.003 } else { 0.0 });
        c.update(dt * 1.01, Some(quantised), &mut out);
        if i > 200 {
            worst = worst.max((c.song_time() - real).abs());
        }
    }
    assert!(worst < 0.015, "locked clock stays within 15 ms of truth, worst {worst}");
    assert_eq!(c.snap_count(), 0);
    // Seek: audio jumps 2 s ahead. Clock must snap, not crawl.
    c.update(dt, Some(real + 2.0), &mut out);
    assert_eq!(c.snap_count(), 1);
    assert!(approx(c.song_time(), real + 2.0, 1e-9));
}

#[test]
fn calibration_offset_shifts_shot_time() {
    let mut c = Conductor::new(ClockConfig::default());
    c.set_input_offset_ms(-18.0);
    assert!(approx(c.shot_time(10.0), 9.982, 1e-9));
    assert!(approx(c.ms_from_nearest_beat(60.0 / 128.0 * 3.0 + 0.020), 20.0, 1e-6));
    assert!(approx(c.ms_from_nearest_beat(60.0 / 128.0 * 3.0 - 0.020), -20.0, 1e-6));
}

// ---------------------------------------------------------------- chart

#[test]
fn warmup_compiles_and_is_sorted() {
    let chart = compile(&warmup_level(128.0), &builtin_lib()).unwrap();
    assert_eq!(chart.targets.len(), 4 * 2 + 4 * 2 + 8 * 2 + 4 + 4 + 1 + 1 + 8 * 2 + 8 + 4);
    for w in chart.targets.windows(2) {
        assert!(w[0].hit_time <= w[1].hit_time);
        assert!(w[0].spawn_time < w[0].hit_time);
    }
    for (i, t) in chart.targets.iter().enumerate() {
        assert_eq!(t.id as usize, i);
    }
    let first = &chart.targets[0];
    assert!(approx(first.hit_beat, 8.0, 1e-9), "lead-in of 8 beats");
    assert!(approx(first.spawn_beat, 7.0, 1e-9));
    assert!(approx(first.hit_time, 8.0 * 60.0 / 128.0, 1e-9));
    let track = chart.targets.iter().find(|t| t.kind == Kind::Track).unwrap();
    assert!(approx(track.end_beat - track.hit_beat, 4.0, 1e-9));
    assert!(track.to.is_some());
    // spawn_order is sorted by spawn_time
    for w in chart.spawn_order.windows(2) {
        assert!(chart.targets[w[0]].spawn_time <= chart.targets[w[1]].spawn_time);
    }
}

#[test]
fn same_chart_at_different_bpm_is_identical_in_beat_space() {
    let a = compile(&warmup_level(90.0), &builtin_lib()).unwrap();
    let b = compile(&warmup_level(174.0), &builtin_lib()).unwrap();
    assert_eq!(a.targets.len(), b.targets.len());
    for (x, y) in a.targets.iter().zip(b.targets.iter()) {
        assert_eq!(x.kind, y.kind);
        assert_eq!(x.aim, y.aim);
        assert!(approx(x.hit_beat, y.hit_beat, 1e-9));
        assert!(approx(x.spawn_beat, y.spawn_beat, 1e-9));
        assert!(approx(x.hit_time / y.hit_time, 174.0 / 90.0, 1e-9));
        assert!(approx(x.radius as f64, y.radius as f64, 1e-6), "no scaling by default");
    }
    assert!(approx(a.total_time / b.total_time, 174.0 / 90.0, 1e-9));
}

#[test]
fn radius_scaling_knob_grows_targets_at_fast_tempo() {
    let mut lvl = warmup_level(174.0);
    lvl.scaling = Scaling { ref_bpm: 128.0, radius_exp: -0.5 };
    let scaled = compile(&lvl, &builtin_lib()).unwrap();
    let plain = compile(&warmup_level(174.0), &builtin_lib()).unwrap();
    let k = (174.0f64 / 128.0).sqrt() as f32;
    for (s, p) in scaled.targets.iter().zip(plain.targets.iter()) {
        assert!((s.radius - p.radius * k).abs() < 1e-5);
    }
}

#[test]
fn level_json_roundtrip_and_unknown_pattern_error() {
    let lvl = warmup_level(140.0);
    let json = lvl.to_json_pretty();
    let back = Level::from_json(&json).unwrap();
    assert_eq!(lvl, back);
    let bad = Level {
        sections: vec![Section { pattern: "does_not_exist".into(), repeats: 1 }],
        ..warmup_level(120.0)
    };
    let err = compile(&bad, &builtin_lib()).unwrap_err();
    assert_eq!(err.to_string(), "unknown pattern 'does_not_exist'");
}

#[test]
fn track_target_interpolates_along_its_path() {
    let chart = compile(&warmup_level(120.0), &builtin_lib()).unwrap();
    let t = chart.targets.iter().find(|t| t.pattern == "track_sweep").unwrap();
    let mid = t.aim_at((t.hit_time + t.end_time) / 2.0);
    assert!(mid.yaw.abs() < 1e-5, "sweep passes through centre at midpoint, got {}", mid.yaw);
    assert_eq!(t.aim_at(t.hit_time - 1.0), t.aim);
    assert_eq!(t.aim_at(t.end_time + 1.0), t.to.unwrap());
}

// ---------------------------------------------------------------- judge

fn two_target_judge() -> Judge {
    let lvl = Level {
        name: "t".into(),
        bpm: 120.0, // 0.5 s per beat
        lead_in: 2.0,
        sections: vec![Section { pattern: "flick_lr".into(), repeats: 1 }],
        scaling: Scaling::default(),
        patterns: vec![],
        audio: String::new(),
        stems: vec![],
        song_offset_s: 0.0,
        drum_stem: -1,
        kit: Default::default(),
    };
    Judge::new(compile(&lvl, &builtin_lib()).unwrap(), Windows::default())
}

#[test]
fn judge_windows_are_milliseconds() {
    let w = Windows::default();
    assert_eq!(w.judge(0.0), Timing::Perfect);
    assert_eq!(w.judge(-30.0), Timing::Perfect);
    assert_eq!(w.judge(31.0), Timing::Great);
    assert_eq!(w.judge(-60.0), Timing::Great);
    assert_eq!(w.judge(99.0), Timing::Good);
    assert_eq!(w.judge(-101.0), Timing::Early);
    assert_eq!(w.judge(150.0), Timing::Late);
}

#[test]
fn judge_spawns_then_hits_then_expires() {
    let mut j = two_target_judge();
    let mut out = Vec::new();
    // Target 0: hit at beat 2 = 1.0 s, spawn at 0.5 s.
    j.advance(0.4, &mut out);
    assert!(out.is_empty());
    j.advance(0.5, &mut out);
    assert_eq!(out, vec![Verdict::Spawn { target: 0 }]);
    out.clear();
    // Hit 20 ms late, 0.1 m off centre on a 0.45 m target.
    let v = j.fire(Shot { time: 1.020, hit: Some(Hit { target: 0, offset_m: 0.1 }) });
    match v {
        Verdict::Hit { target, timing, delta_ms, precision, kind, hit_beat } => {
            assert_eq!(target, 0);
            assert_eq!(kind, Kind::Flick);
            assert_eq!(timing, Timing::Perfect);
            assert!(approx(delta_ms, 20.0, 1e-6));
            assert!((precision - (1.0 - 0.1 / 0.45)).abs() < 1e-5);
            assert!(approx(hit_beat, 2.0, 1e-9));
        }
        other => panic!("expected hit, got {other:?}"),
    }
    // Same target again: consumed.
    assert!(matches!(j.fire(Shot { time: 1.05, hit: Some(Hit { target: 0, offset_m: 0.0 }) }), Verdict::Ignored { .. }));
    // Target 1 hits at 1.5 s; let it expire: window closes at 1.6 s.
    j.advance(1.0, &mut out);
    assert_eq!(out, vec![Verdict::Spawn { target: 1 }]);
    out.clear();
    j.advance(1.6, &mut out);
    assert_eq!(out, vec![Verdict::Spawn { target: 2 }], "target 2 spawns at 1.5 s; target 1 still inside its window");
    out.clear();
    j.advance(1.66, &mut out);
    assert!(out.is_empty(), "late window keeps target 1 alive to 1.66 s");
    j.advance(1.661, &mut out);
    assert_eq!(out, vec![Verdict::Expired { target: 1, kind: Kind::Flick }]);
    // Whiff reports distance to nearest beat.
    let v = j.fire(Shot { time: 2.0 + 0.040, hit: None });
    assert!(matches!(v, Verdict::Whiff { grid_delta_ms } if approx(grid_delta_ms, 40.0, 1e-6)));
}

#[test]
fn early_and_late_consume_the_target_and_count_as_timing_misses() {
    let mut j = two_target_judge();
    let mut out = Vec::new();
    j.advance(0.5, &mut out);
    let v = j.fire(Shot { time: 1.0 - 0.140, hit: Some(Hit { target: 0, offset_m: 0.0 }) });
    assert!(matches!(v, Verdict::Hit { timing: Timing::Early, .. }));
    assert!(!j.is_live(0));
    out.clear();
    j.advance(1.0, &mut out);
    let v = j.fire(Shot { time: 1.5 + 0.120, hit: Some(Hit { target: 1, offset_m: 0.0 }) });
    assert!(matches!(v, Verdict::Hit { timing: Timing::Late, .. }));
}

#[test]
fn tracking_counts_samples_only_inside_the_hold() {
    let lvl = Level {
        name: "t".into(),
        bpm: 120.0,
        lead_in: 0.0,
        sections: vec![Section { pattern: "track_sweep".into(), repeats: 1 }],
        scaling: Scaling::default(),
        patterns: vec![],
        audio: String::new(),
        stems: vec![],
        song_offset_s: 0.0,
        drum_stem: -1,
        kit: Default::default(),
    };
    let mut j = Judge::new(compile(&lvl, &builtin_lib()).unwrap(), Windows::default());
    let mut out = Vec::new();
    j.advance(-0.5, &mut out); // spawn (lead 1 beat = 0.5 s before hit at 0)
    assert_eq!(out, vec![Verdict::Spawn { target: 0 }]);
    assert!(!j.track_sample(0, true), "before hold starts: discarded");
    j.advance(0.0, &mut out);
    for i in 0..16 {
        assert!(j.track_sample(0, i % 4 != 0));
    }
    assert!(matches!(j.fire(Shot { time: 1.0, hit: Some(Hit { target: 0, offset_m: 0.0 }) }), Verdict::Ignored { .. }));
    out.clear();
    j.advance(2.0, &mut out); // end at beat 4 = 2.0 s, not yet past
    assert!(out.is_empty());
    j.advance(2.001, &mut out);
    assert_eq!(out, vec![Verdict::TrackDone { target: 0, on: 12, total: 16 }]);
    assert!(j.is_finished());
}

// ---------------------------------------------------------------- score

#[test]
fn combo_multiplier_and_breaks() {
    let mut s = Session::new(Scoring::default());
    let hit = |timing| Verdict::Hit { target: 0, kind: Kind::Flick, timing, delta_ms: 0.0, precision: 1.0, hit_beat: 0.0 };
    s.apply(&hit(Timing::Perfect));
    assert_eq!(s.combo(), 1);
    assert_eq!(s.score(), 102); // 100 * (1 + 1/50)
    s.apply(&hit(Timing::Great));
    assert_eq!(s.combo(), 2);
    s.apply(&Verdict::Whiff { grid_delta_ms: 0.0 });
    assert_eq!(s.combo(), 0);
    s.apply(&hit(Timing::Good));
    s.apply(&hit(Timing::Late));
    assert_eq!(s.combo(), 0, "off-beat breaks combo");
    s.apply(&Verdict::Expired { target: 9, kind: Kind::Micro });
    let r = s.report();
    assert_eq!((r.shots, r.hits, r.whiffs, r.expired), (5, 4, 1, 1));
    assert_eq!(r.max_combo, 2);
    assert!(approx(r.aim_accuracy, 0.8, 1e-9));
    assert!(approx(r.beat_accuracy, 3.0 / 5.0, 1e-9));
    assert!(approx(r.timing_quality, (1.0 + 0.75 + 0.5) / 5.0, 1e-9));
    assert_eq!(r.by_kind["MICRO"].expired, 1);
}

#[test]
fn precision_floor_rewards_centre_hits() {
    let mut a = Session::new(Scoring::default());
    let mut b = Session::new(Scoring::default());
    a.apply(&Verdict::Hit { target: 0, kind: Kind::Flick, timing: Timing::Perfect, delta_ms: 0.0, precision: 1.0, hit_beat: 0.0 });
    b.apply(&Verdict::Hit { target: 0, kind: Kind::Flick, timing: Timing::Perfect, delta_ms: 0.0, precision: 0.0, hit_beat: 0.0 });
    assert!(approx(b.score() as f64 / a.score() as f64, 0.6, 0.01));
}

// ---------------------------------------------------------------- full simulation

#[test]
fn perfect_bot_is_all_perfect_at_every_bpm() {
    for bpm in [90.0, 128.0, 174.0] {
        let r = run_level(&warmup_level(bpm), Bot::perfect(), SimConfig::default());
        let rep = &r.report;
        let chart = compile(&warmup_level(bpm), &builtin_lib()).unwrap();
        let tracks = chart.targets.iter().filter(|t| t.kind == Kind::Track).count() as u32;
        assert_eq!(rep.hits, chart.targets.len() as u32 - tracks, "bpm {bpm}");
        assert_eq!(rep.perfect, rep.hits);
        assert_eq!(rep.expired, 0);
        assert_eq!(rep.whiffs, 0);
        assert!(approx(rep.track_ratio, 1.0, 1e-9));
        assert!(approx(rep.precision, 1.0, 1e-9));
        assert_eq!(rep.grade, Grade::S);
        assert_eq!(r.spawns, chart.targets.len() as u32);
        assert_eq!(rep.max_combo, rep.hits + tracks);
        // Clock: one tick per sixteenth from pre-roll to the end.
        let expected_ticks = ((r.sim_time_s * bpm / 60.0 * 4.0).floor() + (1.0 * bpm / 60.0 * 4.0).floor()) as u64;
        assert!(r.ticks.abs_diff(expected_ticks) <= 1, "ticks {} vs {}", r.ticks, expected_ticks);
        assert_eq!(r.snaps, 0, "a steady audio clock never snaps");
        assert!(r.rms_drift_ms < 6.0, "rms drift {}", r.rms_drift_ms);
    }
}

#[test]
fn human_bot_distribution_matches_its_jitter() {
    // Gaussian with sd 22 ms and +8 ms bias: P(|d|<=30) ~ 0.80, P(|d|<=100) ~ 1.0
    let r = run_level(&warmup_level(128.0), Bot::human(22.0), SimConfig { seed: 11, ..SimConfig::default() });
    let rep = &r.report;
    let judged = (rep.hits + rep.expired) as f64;
    assert!(rep.hits > 50);
    let perfect_frac = rep.perfect as f64 / rep.hits as f64;
    assert!(perfect_frac > 0.65 && perfect_frac < 0.95, "perfect frac {perfect_frac}");
    assert!(rep.early + rep.late <= 2, "almost nothing outside 100 ms at sd 22");
    assert!(rep.timing_mean_ms > 0.0 && rep.timing_mean_ms < 16.0, "bias recovered: {}", rep.timing_mean_ms);
    assert!(rep.timing_std_ms > 14.0 && rep.timing_std_ms < 30.0, "sd recovered: {}", rep.timing_std_ms);
    assert!(rep.whiffs > 0, "hit_prob < 1 produces whiffs");
    assert!(rep.expired as f64 / judged < 0.08);
    assert!(rep.aim_accuracy > 0.85 && rep.aim_accuracy < 1.0);
    assert!(matches!(rep.grade, Grade::A | Grade::B), "grade {:?}", rep.grade);
}

#[test]
fn late_bot_lands_in_great_and_good() {
    let bot = Bot { bias_ms: 55.0, jitter_ms: 5.0, ..Bot::perfect() };
    let r = run_level(&warmup_level(128.0), bot, SimConfig::default());
    let rep = &r.report;
    assert_eq!(rep.perfect, 0);
    assert!(rep.great + rep.good == rep.hits);
    assert!(rep.late <= 1);
    assert!(rep.timing_mean_ms > 50.0 && rep.timing_mean_ms < 60.0);
}

#[test]
fn simulation_is_deterministic_per_seed() {
    let a = run_level(&warmup_level(150.0), Bot::human(30.0), SimConfig { seed: 99, ..SimConfig::default() });
    let b = run_level(&warmup_level(150.0), Bot::human(30.0), SimConfig { seed: 99, ..SimConfig::default() });
    assert_eq!(a, b);
    let c = run_level(&warmup_level(150.0), Bot::human(30.0), SimConfig { seed: 100, ..SimConfig::default() });
    assert_ne!(a.report.score, c.report.score);
}

#[test]
fn no_audio_clock_runs_open_loop_without_events_lost() {
    let r = run_level(&warmup_level(128.0), Bot::perfect(), SimConfig { audio_clock: false, ..SimConfig::default() });
    assert_eq!(r.report.perfect, r.report.hits);
    assert_eq!(r.snaps, 0);
    assert_eq!(r.rms_drift_ms, 0.0);
}

#[test]
fn game_facade_applies_input_offset_and_song_offset() {
    let lvl = Level {
        name: "t".into(),
        bpm: 120.0,
        lead_in: 2.0,
        sections: vec![Section { pattern: "flick_lr".into(), repeats: 1 }],
        scaling: Scaling::default(),
        patterns: vec![],
        audio: String::new(),
        stems: vec![],
        song_offset_s: 0.0,
        drum_stem: -1,
        kit: Default::default(),
    };
    let cfg = ClockConfig { song_offset_s: 0.25, input_offset_ms: -40.0, ..Default::default() };
    let mut g = Game::from_level(&lvl, cfg, Windows::default(), Scoring::default()).unwrap();
    g.start(0.0);
    // beat 2 at chart time 1.0 s = stream time 1.25 s. Spawn at stream 0.75 s.
    let mut spawned = false;
    let mut t = 0.0;
    while t < 0.8 {
        for e in g.update(0.01, None) {
            if let downbeat_core::GameEvent::Judge(Verdict::Spawn { target: 0 }) = e {
                spawned = true;
                assert!(t >= 0.74, "spawned at stream time {t}");
            }
        }
        t += 0.01;
    }
    assert!(spawned);
    // Click measured at stream 1.29 s; -40 ms calibration makes it 1.25 = dead on.
    let v = g.fire(1.29, Some(Hit { target: 0, offset_m: 0.0 }));
    assert!(matches!(v, Verdict::Hit { timing: Timing::Perfect, delta_ms, .. } if delta_ms.abs() < 1e-6));
}

// ---------------------------------------------------------------- late window

#[test]
fn late_window_then_expiry_is_frame_independent() {
    // Target 0 hits at 1.0 s. good = 100 ms, late = 160 ms.
    let mut j = two_target_judge();
    let mut out = Vec::new();
    j.advance(0.5, &mut out);
    out.clear();
    // 1.12 s: inside the late band → LATE, consumed.
    let v = j.fire(Shot { time: 1.120, hit: Some(Hit { target: 0, offset_m: 0.0 }) });
    assert!(matches!(v, Verdict::Hit { timing: Timing::Late, .. }));
    // Target 1 hits at 1.5 s: expires only after 1.66 s.
    j.advance(1.65, &mut out);
    assert!(!out.iter().any(|v| matches!(v, Verdict::Expired { target: 1, .. })));
    j.advance(1.661, &mut out);
    assert!(out.iter().any(|v| matches!(v, Verdict::Expired { target: 1, .. })));
    // Time never rewinds.
    out.clear();
    j.advance(1.0, &mut out);
    assert!(out.is_empty());
}

#[test]
fn game_fire_advances_judge_to_the_click_so_frame_boundaries_do_not_matter() {
    let lvl = Level {
        name: "t".into(),
        bpm: 120.0,
        lead_in: 2.0,
        sections: vec![Section { pattern: "flick_lr".into(), repeats: 1 }],
        scaling: Scaling::default(),
        patterns: vec![],
        audio: String::new(),
        stems: vec![],
        song_offset_s: 0.0,
        drum_stem: -1,
        kit: Default::default(),
    };
    let mk = || Game::from_level(&lvl, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    // Frame boundary at 1.155 s, click at 1.165 s (target 0 expires at 1.160).
    let mut a = mk();
    a.start(0.0);
    let _ = a.update(1.155, None).count();
    let va = a.fire(1.165, Some(Hit { target: 0, offset_m: 0.0 }));
    // Frame boundary at 1.161 s instead: target already expired at the frame.
    let mut b = mk();
    b.start(0.0);
    let _ = b.update(1.161, None).count();
    let vb = b.fire(1.165, Some(Hit { target: 0, offset_m: 0.0 }));
    assert!(matches!(va, Verdict::Ignored { .. }), "{va:?}");
    assert!(matches!(vb, Verdict::Ignored { .. }), "{vb:?}");
    assert_eq!(a.report().expired, 1);
    assert_eq!(b.report().expired, 1);
    // And a click at 1.150 s is LATE in both.
    let mut c = mk();
    c.start(0.0);
    let _ = c.update(1.149, None).count();
    assert!(matches!(c.fire(1.150, Some(Hit { target: 0, offset_m: 0.0 })), Verdict::Hit { timing: Timing::Late, .. }));
}

// ---------------------------------------------------------------- replay

#[test]
fn replay_reproduces_the_live_report_exactly() {
    for (bpm, bot, seed) in [(128.0, Bot::human(22.0), 5u64), (174.0, Bot::human(40.0), 9), (90.0, Bot::perfect(), 1)] {
        let r = run_level(&warmup_level(bpm), bot, SimConfig { seed, ..SimConfig::default() });
        let replay = r.replay.expect("game built from a level records a replay");
        assert!(replay.inputs.len() >= r.report.shots as usize);
        let json = replay.to_json();
        let back = downbeat_core::replay::Replay::from_json(&json).unwrap();
        let replayed = back.play().unwrap();
        assert_eq!(replayed, r.report, "bpm {bpm} seed {seed}");
    }
}

#[test]
fn report_history_matches_counts() {
    let r = run_level(&warmup_level(128.0), Bot::human(22.0), SimConfig::default());
    let rep = &r.report;
    assert_eq!(rep.history.len(), rep.hits as usize);
    let perfects = rep.history.iter().filter(|h| h.timing == Timing::Perfect).count() as u32;
    assert_eq!(perfects, rep.perfect);
    for w in rep.history.windows(2) {
        assert!(w[0].hit_beat <= w[1].hit_beat);
    }
}

// ---------------------------------------------------------------- analysis

#[test]
fn analyser_recovers_tempo_and_offset_from_synthetic_music() {
    use downbeat_core::analysis::{analyze, synth_track};
    for (bpm, offset, secs) in [(140.0, 0.42, 30.0), (92.0, 0.11, 45.0), (183.5, 0.05, 25.0), (128.0, 0.31, 40.0)] {
        let s = synth_track(bpm, offset, secs, 44_100, 3);
        let a = analyze(&s, 44_100);
        assert!((a.bpm - bpm).abs() < 0.05, "bpm {bpm}: got {}", a.bpm);
        assert!((a.offset_s - offset).abs() < 0.004, "offset {offset}: got {}", a.offset_s);
        assert!(a.tempo_confidence > 4.0 && a.phase_confidence > 4.0);
        assert!(a.bars >= 1.0);
    }
}

#[test]
fn analyser_handles_silence_and_short_input() {
    use downbeat_core::analysis::analyze;
    let a = analyze(&vec![0.0f32; 44_100], 44_100);
    assert_eq!(a.bpm, 0.0);
    let b = analyze(&[0.0f32; 10], 44_100);
    assert_eq!(b.bpm, 0.0);
    assert!(b.onset.is_empty());
}

// ---------------------------------------------------------------- rating

#[test]
fn rating_anchor_and_tempo_monotonic() {
    use downbeat_core::rating::rate;
    let r128 = rate(&compile(&warmup_level(128.0), &builtin_lib()).unwrap());
    assert!((r128.stars - 3.0).abs() < 0.02, "anchor drifted: {}", r128.stars);
    let r90 = rate(&compile(&warmup_level(90.0), &builtin_lib()).unwrap());
    let r174 = rate(&compile(&warmup_level(174.0), &builtin_lib()).unwrap());
    assert!(r90.stars < r128.stars && r128.stars < r174.stars);
    assert!(r174.aim_speed > r90.aim_speed);
    assert!((r174.density / r90.density - 174.0 / 90.0).abs() < 0.01);
    assert_eq!(r128.strain.len(), 68, "one strain value per non-track target");
}

#[test]
fn rating_rises_when_targets_shrink_and_is_zero_for_rest() {
    use downbeat_core::rating::rate;
    // Same patterns at half radius, overriding the builtins inline.
    let mut half = warmup_level(128.0);
    for p in downbeat_core::patterns::builtin_patterns() {
        let mut q = p.clone();
        for e in &mut q.events {
            e.radius *= 0.5;
        }
        half.patterns.push(q);
    }
    let base = rate(&compile(&warmup_level(128.0), &builtin_lib()).unwrap());
    let shrunk = rate(&compile(&half, &builtin_lib()).unwrap());
    assert!(shrunk.stars > base.stars, "{} vs {}", shrunk.stars, base.stars);
    assert!(shrunk.precision > base.precision * 1.9);
    let rest = Level { sections: vec![Section { pattern: "rest".into(), repeats: 4 }], ..warmup_level(128.0) };
    let r = rate(&compile(&rest, &builtin_lib()).unwrap());
    assert_eq!(r.stars, 0.0);
    assert!(r.strain.is_empty());
}

// ---------------------------------------------------------------- section energy

#[test]
fn bar_energy_reads_a_quiet_intro_as_quiet() {
    use downbeat_core::analysis::{analyze, bar_energy, synth_track_sections};
    let s = synth_track_sections(128.0, 0.2, 40.0, 44_100, 3, 6);
    let a = analyze(&s, 44_100);
    assert!((a.bpm - 128.0).abs() < 0.05);
    let e = bar_energy(&a, a.bpm, a.offset_s, 20);
    let quiet: f64 = e[1..5].iter().sum::<f64>() / 4.0;
    let loud: f64 = e[8..18].iter().sum::<f64>() / 10.0;
    assert!(quiet < loud * 0.85, "quiet {quiet} loud {loud}");
    assert!(e.iter().all(|v| (0.0..=1.0).contains(v)));
    assert_eq!(bar_energy(&a, 0.0, 0.0, 3), vec![0.0; 3]);
}

// ---------------------------------------------------------------- fingerprint

#[test]
fn replay_refuses_a_changed_chart() {
    use downbeat_core::replay::ReplayError;
    let r = run_level(&warmup_level(128.0), Bot::perfect(), SimConfig::default());
    let mut replay = r.replay.unwrap();
    assert_eq!(replay.play().unwrap(), r.report);
    // Tamper with the level: one pattern overridden inline with a wider target.
    let mut p = downbeat_core::patterns::builtin_lib()["flick_lr"].clone();
    p.events[0].radius = 0.9;
    replay.level.patterns.push(p);
    match replay.play() {
        Err(ReplayError::ChartMismatch { recorded, current }) => assert_ne!(recorded, current),
        other => panic!("expected mismatch, got {other:?}"),
    }
    // Fingerprint is stable across compiles and differs across tempo.
    let a = compile(&warmup_level(128.0), &builtin_lib()).unwrap().fingerprint();
    let b = compile(&warmup_level(128.0), &builtin_lib()).unwrap().fingerprint();
    let c = compile(&warmup_level(130.0), &builtin_lib()).unwrap().fingerprint();
    assert_eq!(a, b);
    assert_ne!(a, c);
}


// ---------------------------------------------------------------- rush / fill level

#[test]
fn rush_fill_pattern_and_fill_level() {
    use downbeat_core::patterns::fill_level;
    use downbeat_core::rating::rate;
    let lib = builtin_lib();
    let p = &lib["rush_fill"];
    let rushes: Vec<_> = p.events.iter().filter(|e| e.kind == Kind::Rush).collect();
    assert_eq!(rushes.len(), 8);
    for w in rushes.windows(2) {
        assert!(approx(w[1].beat - w[0].beat, 0.25, 1e-9), "sixteenths");
        assert!(w[1].aim.yaw > w[0].aim.yaw, "sweeps left to right");
    }
    let chart = compile(&fill_level(128.0), &lib).unwrap();
    assert!(chart.targets.iter().any(|t| t.kind == Kind::Rush));
    let r = rate(&chart);
    let w = rate(&compile(&warmup_level(128.0), &lib).unwrap());
    assert!(r.stars > w.stars, "fill {} vs warmup {}", r.stars, w.stars);
    // A perfect bot clears it at every tempo and the replay reproduces it.
    for bpm in [100.0, 160.0] {
        let res = run_level(&fill_level(bpm), Bot::perfect(), SimConfig::default());
        assert_eq!(res.report.expired, 0);
        assert_eq!(res.report.perfect, res.report.hits);
        assert_eq!(res.replay.unwrap().play().unwrap(), res.report);
    }
    // Kit / drum-stem fields round-trip through JSON and default sanely.
    let mut lvl = fill_level(140.0);
    lvl.stems = vec!["res://a.ogg".into(), "res://drums.ogg".into()];
    lvl.drum_stem = 1;
    lvl.kit.insert("FLICK".into(), "res://kick.wav".into());
    let back = Level::from_json(&lvl.to_json_pretty()).unwrap();
    assert_eq!(back, lvl);
    let bare = Level::from_json(r#"{"name":"x","bpm":120,"sections":[{"pattern":"rest","repeats":1}]}"#).unwrap();
    assert_eq!(bare.drum_stem, -1);
    assert!(bare.kit.is_empty());
}
