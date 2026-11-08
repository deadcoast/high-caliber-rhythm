use downbeat_core::chart::{compile, Kind, Level, Scaling, Section, Segment};
use downbeat_core::clock::{ClockConfig, ClockEvent, Conductor};
use downbeat_core::judge::{Hit, Judge, Shot, Timing, Verdict, Windows};
use downbeat_core::patterns::{builtin_lib, warmup_level};
use downbeat_core::score::{Grade, Scoring, Session};
use downbeat_core::sim::{run_game, run_level, Bot, SimConfig};
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
        ..Level::default()
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
        ..Level::default()
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
    let r = s.report(3.0, downbeat_core::mods::Mods::NONE);
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
        ..Level::default()
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
        ..Level::default()
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
    lvl.kit.insert("FLICK".into(), "res://kick.wav".into());
    let back = Level::from_json(&lvl.to_json_pretty()).unwrap();
    assert_eq!(back, lvl);
    let bare = Level::from_json(r#"{"name":"x","bpm":120,"sections":[{"pattern":"rest","repeats":1}]}"#).unwrap();
    assert!(bare.kit.is_empty());
    assert!(bare.segments.is_empty() && bare.rooms.is_empty());
}

// ---------------------------------------------------------------- rooms / segments / damage

#[test]
fn street_level_lays_rooms_out_along_the_path() {
    use downbeat_core::chart::{Segment, TRAVEL_FILL, TRAVEL_SPEED};
    use downbeat_core::patterns::street_level;
    let lvl = street_level(128.0);
    let chart = compile(&lvl, &builtin_lib()).unwrap();
    assert_eq!(chart.rooms.len(), 3);
    assert_eq!(chart.rooms.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["street", "atrium", "platform"]);
    let spb = 60.0 / 128.0;
    // First traversal is 1 bar; street room is 40 deep, centred on its origin.
    let corridor0 = (1.0 * 4.0 * spb) as f32 * TRAVEL_SPEED * TRAVEL_FILL;
    let street = chart.room("street").unwrap();
    assert!((street.origin.unwrap()[2] + corridor0 + 20.0).abs() < 1e-3, "{:?}", street.origin);
    // Rooms never overlap and go further down -Z in path order.
    for w in chart.rooms.windows(2) {
        let a = w[0].origin.unwrap()[2] - w[0].size[2] * 0.5;
        let b = w[1].origin.unwrap()[2] + w[1].size[2] * 0.5;
        assert!(b < a, "{} then {}", w[0].name, w[1].name);
    }
    // Spans are contiguous from the lead-in to the end.
    assert_eq!(chart.spans.len(), lvl.segments.len());
    assert!(approx(chart.spans[0].start_beat, 8.0, 1e-9));
    for w in chart.spans.windows(2) {
        assert!(approx(w[0].end_beat, w[1].start_beat, 1e-9));
    }
    assert!(chart.spans.iter().any(|s| s.kind == "traversal") && chart.spans.iter().any(|s| s.kind == "encounter"));
    // Every target sits inside its room's box and above its floor.
    for t in &chart.targets {
        let r = chart.room(&t.room).unwrap();
        let o = r.origin.unwrap();
        assert!((t.pos[0] - o[0]).abs() <= r.size[0] * 0.5 + 0.01, "{} x {}", t.room, t.pos[0]);
        assert!((t.pos[2] - o[2]).abs() <= r.size[2] * 0.5 + 0.01, "{} z {}", t.room, t.pos[2]);
        assert!(t.pos[1] > o[1] && t.pos[1] < o[1] + r.size[1], "{} y {}", t.room, t.pos[1]);
        assert!(matches!(lvl.segments[t.segment as usize], Segment::Encounter { .. }));
    }
    // Rooms turned by yaw rotate target positions.
    let mut turned = street_level(128.0);
    turned.rooms[0].yaw_deg = 90.0;
    let c2 = compile(&turned, &builtin_lib()).unwrap();
    let a = chart.targets.iter().find(|t| t.room == "street" && t.aim.yaw.abs() > 1.0).unwrap();
    let b = &c2.targets[a.id as usize];
    assert!((a.pos[0] - b.pos[0]).abs() > 1.0 || (a.pos[2] - b.pos[2]).abs() > 1.0);
    assert!(approx(a.pos[1] as f64, b.pos[1] as f64, 1e-4));
}

#[test]
fn default_range_room_keeps_warmup_relative_geometry() {
    let chart = compile(&warmup_level(128.0), &builtin_lib()).unwrap();
    assert_eq!(chart.rooms.len(), 1);
    let stand = chart.rooms[0].stand_world();
    for t in &chart.targets {
        let dx = t.pos[0] - stand[0];
        let dz = t.pos[2] - stand[2];
        let horizontal = (dx * dx + dz * dz).sqrt();
        let expect_h = t.aim.dist * t.aim.pitch.to_radians().cos();
        assert!((horizontal - expect_h).abs() < 1e-3, "{horizontal} vs {expect_h}");
        let authored_y = stand[1] + 1.6 + t.aim.dist * t.aim.pitch.to_radians().sin();
        let expect_y = authored_y.max(stand[1] + 0.6);
        assert!((t.pos[1] - expect_y).abs() < 1e-3, "{} vs {}", t.pos[1], expect_y);
        assert!(t.pos[2] < stand[2], "targets are down the lane");
    }
}

#[test]
fn surviving_enemies_fire_and_hp_reaches_down() {
    use downbeat_core::judge::Verdict;
    // A bot that never shoots, with no-fail on so the run plays through: every
    // enemy expires and fires.
    let bot = Bot { skip_prob: 1.0, track_on_prob: 0.0, ..Bot::perfect() };
    let nf = Game::from_level(&warmup_level(128.0), ClockConfig::default(), Windows::default(), Scoring { no_fail: true, ..Scoring::default() }).unwrap();
    let r = downbeat_core::sim::run_game(nf, bot, SimConfig::default());
    let rep = &r.report;
    let total_damage: u32 = compile(&warmup_level(128.0), &builtin_lib()).unwrap().targets.iter().filter(|t| t.kind != Kind::Track).map(|t| t.kind.damage()).sum();
    assert_eq!(rep.expired, 68);
    assert_eq!(rep.damage_taken, total_damage);
    assert!(rep.downs >= 5, "downs {}", rep.downs);
    let damages = r.verdicts.iter().filter(|v| matches!(v, Verdict::Damage { .. })).count() as u32;
    let downs = r.verdicts.iter().filter(|v| matches!(v, Verdict::Down { .. })).count() as u32;
    assert_eq!(damages + downs, 68, "one follow-up per expiry");
    assert_eq!(downs, rep.downs);
    // Damage events report decreasing hp between downs.
    let mut last_hp = 100;
    for v in &r.verdicts {
        match v {
            Verdict::Damage { hp, amount, .. } => {
                assert!(*hp < last_hp || last_hp == 100, "{hp} after {last_hp}");
                assert!(*amount > 0);
                last_hp = *hp;
            }
            Verdict::Down { .. } => last_hp = 100,
            _ => {}
        }
    }
    // Replay reproduces hp, damage and downs exactly.
    assert_eq!(r.replay.unwrap().play().unwrap(), r.report);
    // Perfect play takes no damage and keeps full hp.
    let p = run_level(&warmup_level(128.0), Bot::perfect(), SimConfig::default()).report;
    assert_eq!((p.damage_taken, p.downs, p.hp), (0, 0, 100));
}

#[test]
fn street_level_is_playable_at_any_tempo() {
    use downbeat_core::patterns::street_level;
    for bpm in [100.0, 128.0, 170.0] {
        let r = run_level(&street_level(bpm), Bot::perfect(), SimConfig::default());
        assert_eq!(r.report.expired, 0);
        assert_eq!(r.report.perfect, r.report.hits);
        assert_eq!(r.report.downs, 0);
        assert_eq!(r.replay.unwrap().play().unwrap(), r.report);
    }
    let json = street_level(128.0).to_json_pretty();
    let back = Level::from_json(&json).unwrap();
    assert_eq!(back, street_level(128.0));
}

// ---------------------------------------------------------------- section restart

#[test]
fn down_restarts_the_section_from_its_checkpoint() {
    use downbeat_core::judge::Verdict;
    use downbeat_core::patterns::street_level;
    use downbeat_core::GameEvent;
    let lvl = street_level(128.0);
    let mut g = Game::from_level(&lvl, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    g.start(-1.0);
    let chart = g.chart().clone();
    let bar = 4.0 * 60.0 / 128.0;
    // Encounter segments are 1 (street), 3 (atrium), 5 (platform).
    let street: Vec<_> = chart.targets.iter().filter(|t| t.segment == 1).cloned().collect();
    let atrium: Vec<_> = chart.targets.iter().filter(|t| t.segment == 3).cloned().collect();
    let atrium_first_spawn = atrium.iter().map(|t| t.spawn_time).fold(f64::INFINITY, f64::min);
    let dt = 1.0 / 120.0;
    let mut t = -1.0;
    let mut restart: Option<Verdict> = None;
    let mut score_after_street = 0u64;
    let mut spawns_atrium_before = 0;
    let mut spawns_atrium_after = 0;
    while t < chart.total_time + 1.0 {
        let evs: Vec<GameEvent> = g.update(dt, None).collect();
        t += dt;
        for e in &evs {
            if let GameEvent::Judge(v) = e {
                match v {
                    Verdict::Restart { .. } => {
                        restart = Some(v.clone());
                        t = g.conductor().song_time();
                    }
                    Verdict::Spawn { target } if atrium.iter().any(|a| a.id == *target) => {
                        if restart.is_none() { spawns_atrium_before += 1 } else { spawns_atrium_after += 1 }
                    }
                    _ => {}
                }
            }
        }
        let ct = g.now_chart_time();
        // Kill every street enemy and every platform enemy on the beat; let the
        // atrium go unshot the first time through, shoot it after the restart.
        for tg in chart.targets.iter().filter(|x| x.kind != Kind::Track) {
            let shoot_now = tg.hit_time <= ct && tg.hit_time > ct - dt;
            let allowed = tg.segment != 3 || restart.is_some();
            if shoot_now && allowed && g.judge().is_live(tg.id) {
                g.fire(tg.hit_time, Some(Hit { target: tg.id, offset_m: 0.0 }));
            }
        }
        for tg in chart.targets.iter().filter(|x| x.kind == Kind::Track) {
            if g.judge().is_live(tg.id) {
                g.track_sample(tg.id, true);
            }
        }
        if restart.is_none() && !street.is_empty() && ct > street.iter().map(|x| x.hit_time).fold(0.0, f64::max) + 0.3 && score_after_street == 0 {
            score_after_street = g.session().score();
        }
        if g.is_finished() {
            break;
        }
    }
    let r = restart.expect("a down in the atrium restarts it");
    match r {
        Verdict::Restart { segment, chart_time } => {
            assert_eq!(segment, 3);
            assert!(approx(chart_time, atrium_first_spawn - bar, 1e-9));
        }
        _ => unreachable!(),
    }
    assert!(spawns_atrium_before >= 1 && spawns_atrium_after == atrium.len(), "{spawns_atrium_before} / {spawns_atrium_after}");
    let rep = g.report();
    assert_eq!(rep.downs, 1);
    assert_eq!(g.restarts(), 1);
    // The failed attempt's expiries are gone from the stats; only the retry counts.
    assert_eq!(rep.hits as usize, chart.targets.iter().filter(|x| x.kind != Kind::Track).count());
    assert_eq!(rep.perfect, rep.hits);
    assert!(rep.score > score_after_street && score_after_street > 0);
    assert_eq!(rep.hp, 100);
    assert!(g.is_finished());
}

#[test]
fn failing_bot_restarts_until_it_clears_and_replay_matches() {
    let bot = Bot { skip_prob: 0.35, ..Bot::perfect() };
    let r = run_level(&warmup_level(128.0), bot, SimConfig { seed: 21, ..SimConfig::default() });
    assert!(r.restarts >= 1, "restarts {}", r.restarts);
    assert_eq!(r.report.downs, r.restarts);
    // Only the clearing attempt's misses remain in the stats, and never enough to down.
    assert!(r.report.expired < 68 / 2, "expired {}", r.report.expired);
    assert_eq!(r.replay.unwrap().play().unwrap(), r.report);
}

#[test]
fn no_fail_mod_takes_downs_without_restarting() {
    let bot = Bot { skip_prob: 1.0, track_on_prob: 0.0, ..Bot::perfect() };
    let lvl = warmup_level(128.0);
    let game = Game::from_level(&lvl, ClockConfig::default(), Windows::default(), Scoring { no_fail: true, ..Scoring::default() }).unwrap();
    let r = downbeat_core::sim::run_game(game, bot, SimConfig::default());
    assert_eq!(r.restarts, 0);
    assert!(r.report.downs >= 5);
    assert_eq!(r.report.expired, 68);
}

#[test]
fn never_killing_bot_hits_the_wall_clock_cap() {
    let bot = Bot { skip_prob: 1.0, track_on_prob: 0.0, ..Bot::perfect() };
    let r = run_level(&warmup_level(128.0), bot, SimConfig { max_wall_s: 120.0, ..SimConfig::default() });
    assert!(r.restarts >= 2);
    assert!(r.sim_time_s < 60.0, "kept rewinding, {}", r.sim_time_s);
}

// ---------------------------------------------------------------- range drills

#[test]
fn drills_fill_the_requested_minutes_and_rank_bands_are_stable() {
    use downbeat_core::patterns::{drill_level, drills};
    use downbeat_core::rating::{bpm_band, rank};
    assert!(drills().len() >= 6);
    for d in drills() {
        for (bpm, minutes) in [(100.0, 1.0), (140.0, 2.0), (180.0, 3.0)] {
            let lvl = drill_level(&d.name, bpm, minutes).unwrap();
            let chart = compile(&lvl, &builtin_lib()).unwrap();
            let target = minutes * 60.0;
            assert!((chart.total_time - target).abs() < target * 0.25 + 15.0, "{} {bpm} {minutes}: {}", d.name, chart.total_time);
            assert_eq!(chart.rooms.len(), 1);
        }
    }
    assert!(drill_level("nope", 128.0, 1.0).is_none());
    assert_eq!(rank(0.96), "DIAMOND");
    assert_eq!(rank(0.80), "GOLD");
    assert_eq!(rank(0.10), "IRON");
    assert_eq!(bpm_band(128.0), "120-139");
    assert_eq!(bpm_band(140.0), "140-159");
    let r = run_level(&drill_level("switch", 150.0, 1.0).unwrap(), Bot::perfect(), SimConfig::default());
    assert!(approx(r.report.composite, 1.0, 1e-9));
    assert_eq!(r.report.expired, 0);
}

// ---------------------------------------------------------------- osu layer: mods, pp, fc, integrity, volley

#[test]
fn mods_change_chart_fingerprint_and_scale_score() {
    use downbeat_core::mods::Mods;
    let lvl = warmup_level(128.0);
    let nm = Game::from_level(&lvl, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    let dt = Game::from_level_with_mods(&lvl, Mods { dt: true, ..Mods::NONE }, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    let hr = Game::from_level_with_mods(&lvl, Mods { hr: true, ..Mods::NONE }, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    assert!(approx(dt.chart().bpm, 192.0, 1e-9));
    assert!(approx(dt.conductor().config().bpm, 192.0, 1e-9));
    assert_ne!(nm.chart().fingerprint(), dt.chart().fingerprint());
    assert_ne!(nm.chart().fingerprint(), hr.chart().fingerprint());
    assert!((hr.chart().targets[0].radius - nm.chart().targets[0].radius * 0.75).abs() < 1e-6);
    assert_eq!(Mods::from_label("DTHRHD").label(), "DTHRHD");
    assert_eq!(Mods::from_label("nm").label(), "NM");
    assert_eq!(Mods::from_label("DTHT").label(), "NM", "DT and HT cancel");
    let r_nm = downbeat_core::sim::run_game(nm, Bot::perfect(), SimConfig::default()).report;
    let r_dt = downbeat_core::sim::run_game(dt, Bot::perfect(), SimConfig::default()).report;
    assert_eq!(r_nm.mods, "NM");
    assert_eq!(r_dt.mods, "DT");
    assert!(approx(r_dt.mod_multiplier, 1.12, 1e-9));
    assert_eq!(r_dt.score, (r_dt.raw_score as f64 * 1.12).round() as u64);
    assert!(r_dt.stars > r_nm.stars, "DT is the harder chart");
    assert!(r_dt.pp > r_nm.pp);
    // Modded replays carry their mods and re-score identically.
    let dt2 = Game::from_level_with_mods(&lvl, Mods { dt: true, hr: true, ..Mods::NONE }, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    let r = downbeat_core::sim::run_game(dt2, Bot::human(20.0), SimConfig::default());
    let rep = r.replay.unwrap();
    assert_eq!(rep.mods.label(), "DTHR");
    assert_eq!(rep.play().unwrap(), r.report);
}

#[test]
fn full_combo_flags_and_performance_shape() {
    use downbeat_core::rating::{performance, profile_rating};
    let p = run_level(&warmup_level(128.0), Bot::perfect(), SimConfig::default()).report;
    assert!(p.fc && p.pfc);
    assert!(approx(p.composite, 1.0, 1e-9));
    let late = Bot { bias_ms: 45.0, jitter_ms: 3.0, ..Bot::perfect() };
    let g = run_level(&warmup_level(128.0), late, SimConfig::default()).report;
    assert!(g.fc && !g.pfc, "all Great is an FC, not a PFC");
    let h = run_level(&warmup_level(128.0), Bot::human(22.0), SimConfig::default()).report;
    assert!(!h.fc);
    assert!(p.pp > g.pp && g.pp > h.pp, "{} {} {}", p.pp, g.pp, h.pp);
    // Shape: harder chart worth more at equal accuracy; accuracy cubed.
    assert!(performance(4.0, 1.0, 1.0, 1.0) > performance(2.0, 1.0, 1.0, 1.0) * 3.9);
    assert!(approx(performance(3.0, 0.5, 1.0, 1.0) / performance(3.0, 1.0, 1.0, 1.0), 0.125, 1e-9));
    assert_eq!(performance(0.0, 1.0, 1.0, 1.0), 0.0);
    assert!(approx(profile_rating(&[100.0, 100.0, 100.0]), 100.0 + 95.0 + 90.25, 1e-9));
    assert!(approx(profile_rating(&[]), 0.0, 1e-9));
    assert!(approx(profile_rating(&[50.0, 100.0]), 100.0 + 47.5, 1e-9), "sorted descending before weighting");
}

#[test]
fn integrity_flags_bots_and_clears_humans() {
    use downbeat_core::integrity::inspect;
    let perfect = run_level(&warmup_level(128.0), Bot::perfect(), SimConfig::default());
    let i = inspect(&perfect.report, &perfect.replay.clone().unwrap());
    assert!(!i.clean(), "a zero-spread, dead-centre run is flagged");
    assert!(i.flags.iter().any(|f| f.contains("timing spread")));
    assert!(i.flags.iter().any(|f| f.contains("precision")));
    assert!(i.flags.iter().any(|f| f.contains("beat multiples")));
    let human = run_level(&warmup_level(128.0), Bot::human(22.0), SimConfig { seed: 3, ..SimConfig::default() });
    let j = inspect(&human.report, &human.replay.clone().unwrap());
    assert!(j.clean(), "{:?}", j.flags);
    assert_eq!(j.hits_checked, human.report.hits);
}

#[test]
fn downbeat_volley_charges_on_kills_and_fires_on_the_bar() {
    use downbeat_core::judge::{Timing, Verdict};
    use downbeat_core::VOLLEY_FULL;
    let lvl = warmup_level(120.0); // 0.5 s per beat, 2 s per bar
    let mut g = Game::from_level(&lvl, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    g.start(0.0);
    let chart = g.chart().clone();
    // Kill the first 16 enemies on the beat: charge fills.
    let mut killed = 0;
    let dt = 1.0 / 120.0;
    while killed < VOLLEY_FULL {
        let _ = g.update(dt, None).count();
        let ct = g.now_chart_time();
        for tg in chart.targets.iter().filter(|x| x.kind != Kind::Track) {
            if tg.hit_time <= ct && tg.hit_time > ct - dt && g.judge().is_live(tg.id) {
                g.fire(tg.hit_time, Some(Hit { target: tg.id, offset_m: 0.0 }));
                killed += 1;
            }
        }
    }
    assert!(g.volley_ready());
    assert_eq!(g.volley_charge(), VOLLEY_FULL);
    // Switch pair at beats 24..27.5 (targets 16..23): advance to their exposure, mark the live ones.
    let pair: Vec<_> = chart.targets.iter().filter(|x| x.pattern == "switch_pair").take(4).cloned().collect();
    // Two of the pair are exposed once the second has spawned (eighth-note leads).
    while g.now_chart_time() < pair[1].spawn_time + 0.05 {
        let _ = g.update(dt, None).count();
    }
    let live: Vec<u32> = pair.iter().filter(|x| g.judge().is_live(x.id)).map(|x| x.id).collect();
    assert!(live.len() >= 2, "{live:?}");
    for id in &live {
        assert!(g.mark(*id));
    }
    assert!(!g.mark(live[0]), "no double marks");
    assert_eq!(g.marked().len(), live.len());
    // Release exactly on the next bar line after now.
    let bar = 2.0;
    let now = g.now_chart_time();
    let bar_time = (now / bar).ceil() * bar;
    let v = g.release(bar_time + 0.010);
    match v {
        Verdict::Volley { timing, delta_ms, kills } => {
            assert_eq!(timing, Timing::Perfect);
            assert!(approx(delta_ms, 10.0, 1e-6));
            assert_eq!(kills.len(), live.len());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(g.volley_charge(), 0);
    for id in &live {
        assert!(!g.judge().is_live(*id));
    }
    let rep = g.report();
    assert_eq!(rep.hits as usize, 16 + live.len());
    // Off-beat release with a full charge loses everything and breaks combo.
    let mut g2 = Game::from_level(&lvl, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    g2.start(0.0);
    assert!(!g2.mark(0), "no charge, no marks");
    let miss = g2.release(0.7);
    assert!(matches!(miss, Verdict::VolleyMiss { marked: 0, .. }));
    // Replays reproduce volleys.
    let replay = g.replay().unwrap();
    let back = replay.play().unwrap();
    assert_eq!(back.hits, rep.hits);
    assert_eq!(back.score, rep.score);
}


// ---------------------------------------------------------------- audio file path + auto level

#[test]
fn auto_level_from_a_decoded_file_round_trips() {
    use downbeat_core::analysis::{analyze, auto_level, bar_energy, read_audio_mono, synth_track_sections};
    use downbeat_core::rating::rate;
    let dir = std::env::temp_dir().join(format!("downbeat_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.wav");
    let samples = synth_track_sections(132.0, 0.3, 40.0, 44_100, 5, 4);
    let spec = hound::WavSpec { channels: 2, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for s in &samples {
        let v = (s * 32000.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
    let (mono, sr) = read_audio_mono(path.to_str().unwrap()).unwrap();
    assert_eq!(sr, 44_100);
    assert_eq!(mono.len(), samples.len());
    let a = analyze(&mono, sr);
    assert!((a.bpm - 132.0).abs() < 0.05, "{}", a.bpm);
    assert!((a.offset_s - 0.3).abs() < 0.005, "{}", a.offset_s);
    let bars = (((a.duration_s - a.offset_s) / (60.0 / a.bpm)) / 4.0).floor() as usize;
    let energy = bar_energy(&a, a.bpm, a.offset_s, bars);
    let lvl = auto_level("song", "res://audio/song.wav", a.bpm, a.offset_s, a.duration_s, &energy);
    assert_eq!(lvl.difficulty, "Auto");
    assert!(!lvl.sections.is_empty());
    let total_bars: u32 = lvl.sections.iter().map(|s| s.repeats).sum();
    assert_eq!(total_bars as usize, bars.saturating_sub(3).max(1));
    // Quiet intro bars land on the quiet tiers.
    assert!(matches!(lvl.sections[0].pattern.as_str(), "rest" | "micro_walk" | "micro_zigzag" | "flick_lr"), "{}", lvl.sections[0].pattern);
    let chart = compile(&lvl, &builtin_lib()).unwrap();
    assert!(rate(&chart).stars > 0.0);
    let back = Level::from_json(&lvl.to_json_pretty()).unwrap();
    assert_eq!(back, lvl);
    assert!(read_audio_mono(dir.join("missing.mp3").to_str().unwrap()).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------- watching

/// The engine watches a replay by stepping the model in frames and, before
/// each frame, applying every recorded input whose `at` falls inside the
/// frame (judge seeked to `at`, then the input) — the driver's `_feed_replay`.
/// A frame that crosses an expiry before the recorded Advance time downs the
/// model on its own; the inputs up to that Advance are then dropped. A run
/// with restarts watched this way at an uneven frame rate ends with the live
/// report. Returns (applied restarts, spontaneous restarts).
fn watch(live_frame_dt: f64, watch_dt: impl Fn(u64) -> f64) -> (u32, u32) {
    use downbeat_core::replay::Input;
    let bot = Bot { skip_prob: 0.35, ..Bot::perfect() };
    let live = run_level(&warmup_level(128.0), bot, SimConfig { seed: 21, frame_dt: live_frame_dt, ..SimConfig::default() });
    assert!(live.restarts >= 1);
    let replay = live.replay.clone().unwrap();
    let mut g = Game::from_level_with_mods(&replay.level, replay.mods, replay.clock, replay.windows, replay.scoring).unwrap();
    g.start(-2.0);
    let mut index = 0;
    let mut frame = 0u64;
    let mut wall = 0.0;
    let mut advance_pending = false;
    let (mut applied, mut spontaneous) = (0u32, 0u32);
    while !g.is_finished() && wall < 3600.0 {
        let dt = watch_dt(frame);
        let mut horizon = g.now_chart_time() + dt * 1.25 + 0.002;
        while index < replay.inputs.len() && replay.inputs[index].at() <= horizon {
            let inp = replay.inputs[index];
            index += 1;
            if matches!(inp, Input::Advance { .. }) {
                advance_pending = true;
            }
            g.advance_judge(inp.at());
            match inp {
                Input::Shot { raw_time, target, offset_m, .. } => {
                    let hit = if target >= 0 { Some(Hit { target: target as u32, offset_m }) } else { None };
                    g.fire(raw_time, hit);
                }
                Input::Track { target, on, .. } => {
                    g.track_sample(target, on);
                }
                Input::Mark { target, .. } => {
                    g.mark(target);
                }
                Input::Release { raw_time, .. } => {
                    g.release(raw_time);
                }
                Input::Advance { .. } => {}
            }
            horizon = g.now_chart_time() + dt * 1.25 + 0.002;
        }
        let events: Vec<_> = g.update(dt, None).collect();
        for e in &events {
            if let downbeat_core::GameEvent::Judge(Verdict::Restart { .. }) = e {
                if advance_pending {
                    advance_pending = false;
                    applied += 1;
                } else {
                    spontaneous += 1;
                    while index < replay.inputs.len() {
                        let was_advance = matches!(replay.inputs[index], Input::Advance { .. });
                        index += 1;
                        if was_advance {
                            break;
                        }
                    }
                }
            }
        }
        wall += dt;
        frame += 1;
    }
    assert_eq!(index, replay.inputs.len(), "every input applied");
    assert_eq!(g.restarts(), live.restarts);
    assert_eq!(g.report(), live.report);
    (applied, spontaneous)
}

#[test]
fn watching_a_replay_frame_by_frame_reproduces_the_live_report() {
    // Live at 240 fps, watched at 4..40 ms frames with a 120 ms hitch every 97th.
    let (applied, spontaneous) = watch(1.0 / 240.0, |frame| if frame % 97 == 0 { 0.120 } else { 0.004 + 0.036 * ((frame * 7919) % 101) as f64 / 100.0 });
    assert!(applied >= 1);
    let _ = spontaneous;
}

#[test]
fn watching_at_a_higher_frame_rate_than_the_live_run_reproduces_it() {
    // Live at 30 fps (a 33 ms window between an expiry and its recorded
    // Advance), watched at 240 fps: frames land inside that window and the
    // model downs on its own before the Advance arrives.
    let (_applied, spontaneous) = watch(1.0 / 30.0, |_| 1.0 / 240.0);
    assert!(spontaneous >= 1, "the spontaneous-down path was not exercised");
}

// ---------------------------------------------------------------- any song audio

#[test]
fn grid_click_lands_on_the_beats_and_nowhere_else() {
    use downbeat_core::analysis::click_grid_pcm16;
    let sr = 44100u32;
    let (bpm, offset) = (120.0, 0.25);
    let pcm = click_grid_pcm16(bpm, offset, 4.0, sr);
    assert_eq!(pcm.len(), 4 * sr as usize * 2);
    let sample = |t: f64| -> f64 {
        let i = (t * sr as f64) as usize * 2;
        i16::from_le_bytes([pcm[i], pcm[i + 1]]) as f64 / 32767.0
    };
    let rms = |from: f64, to: f64| -> f64 {
        let n = ((to - from) * sr as f64) as usize;
        (0..n).map(|k| sample(from + k as f64 / sr as f64).powi(2)).sum::<f64>().sqrt() / (n as f64).sqrt()
    };
    for b in 0..7 {
        let t = offset + b as f64 * 0.5;
        assert!(rms(t, t + 0.02) > 0.1, "beat {b} silent");
        assert!(rms(t + 0.12, t + 0.45) < 1e-3, "between beats {b} not silent");
    }
    // Before beat 0: silence.
    assert!(rms(0.0, offset - 0.01) < 1e-3);
}

#[test]
fn decoded_audio_folds_to_stereo_pcm16() {
    use downbeat_core::analysis::Decoded;
    let four = Decoded { samples: vec![0.5, -0.5, 1.0, -1.0, 0.25, 0.25, 0.0, 0.0], sample_rate: 48000, channels: 4 };
    let (bytes, ch) = four.pcm16();
    assert_eq!(ch, 2);
    assert_eq!(bytes.len(), 2 * 2 * 2);
    let s = |i: usize| i16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]) as f64 / 32767.0;
    // Frame 0: L = ch0 + 0.5*ch2 + 0.5*ch3 over 1 = 0.5 + 0.5 - 0.5 = 0.5; R = ch1 + 0.5*ch2 + 0.5*ch3 = -0.5.
    assert!((s(0) - 0.5).abs() < 1e-3 && (s(1) + 0.5).abs() < 1e-3);
    let mono = Decoded { samples: vec![0.1, 0.2], sample_rate: 44100, channels: 1 };
    assert_eq!(mono.pcm16().1, 1);
    assert_eq!(mono.mono().len(), 2);
    let stereo = Decoded { samples: vec![1.0, 0.0, 0.0, 1.0], sample_rate: 44100, channels: 2 };
    assert_eq!(stereo.mono(), vec![0.5, 0.5]);
}

// ---------------------------------------------------------------- the produced track

#[test]
fn night_market_renders_on_its_grid_and_its_chart_plays() {
    use downbeat_core::analysis::analyze;
    use downbeat_core::song::{mix, plan, render, total_bars, SAMPLE_RATE};
    assert_eq!(total_bars(), 88);
    assert_eq!(plan().iter().filter(|s| s.lead).count(), 3);
    let stems = render(132.0);
    assert_eq!(stems.len(), 4);
    let n = stems[0].1.len();
    assert!(stems.iter().all(|(_, s)| s.len() == n));
    assert!((n as f64 / SAMPLE_RATE as f64 - (88.0 * 4.0 * 60.0 / 132.0 + 2.0)).abs() < 0.01);
    for (name, s) in &stems {
        let peak = s.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak > 0.5 && peak <= 1.0, "{name} peak {peak}");
    }
    // The intro has no percussion; the chorus has everything.
    let spb = 60.0 / 132.0;
    let rms = |s: &[f32], from_bar: f64, bars: f64| {
        let a = (from_bar * 4.0 * spb * SAMPLE_RATE as f64) as usize;
        let b = ((from_bar + bars) * 4.0 * spb * SAMPLE_RATE as f64) as usize;
        (s[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32).sqrt()
    };
    assert!(rms(&stems[1].1, 0.0, 8.0) < 1e-4, "perc in the intro");
    assert!(rms(&stems[3].1, 0.0, 8.0) < 1e-4, "lead in the intro");
    assert!(rms(&stems[3].1, 32.0, 16.0) > 0.03, "lead in the chorus");
    assert!(rms(&stems[0].1, 48.0, 8.0) < rms(&stems[0].1, 64.0, 16.0) * 0.5, "the break drops the base");
    // The analyser reads the grid back from the mix.
    let a = analyze(&mix(&stems), SAMPLE_RATE);
    assert!((a.bpm - 132.0).abs() < 0.2, "bpm {}", a.bpm);
    assert!(a.offset_s.min((spb - a.offset_s).abs()) < 0.015, "offset {}", a.offset_s);
    // The hand chart compiles against the plan and a human bot clears it.
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../godot/levels/night_market_132.json")).unwrap();
    let level = Level::from_json(&text).unwrap();
    let chart = compile(&level, &builtin_lib()).unwrap();
    assert!(chart.targets.len() > 300);
    assert!(chart.total_beats <= 88.0 * 4.0, "chart runs past the song: {} beats", chart.total_beats);
    // Encounters sit on the sections: street 9-30, atrium 33-48, platform 55-80 (1-based bars).
    let enc: Vec<(String, f64, f64)> = chart.spans.iter().filter(|s| s.kind == "encounter").map(|s| (s.room.clone(), s.start_beat / 4.0 + 1.0, s.end_beat / 4.0 + 1.0)).collect();
    assert_eq!(enc, vec![("street".to_string(), 9.0, 31.0), ("atrium".to_string(), 33.0, 49.0), ("platform".to_string(), 55.0, 81.0)]);
    let r = run_level(&level, Bot::human(22.0), SimConfig { seed: 7, ..SimConfig::default() });
    assert_eq!(r.report.downs, 0, "the human bot went down");
    assert!(r.report.stars > 3.5 && r.report.stars < 5.0, "stars {}", r.report.stars);
}

// ---------------------------------------------------------------- act one

fn level_file(name: &str) -> Level {
    let text = std::fs::read_to_string(format!("{}/../godot/levels/{name}.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Level::from_json(&text).unwrap()
}

#[test]
fn act_one_levels_sit_on_their_songs_and_a_human_clears_them() {
    use downbeat_core::song::track;
    for (name, song, rooms) in [
        ("night_market_132", "night_market", vec!["street", "atrium", "platform"]),
        ("undertow_150", "undertow", vec!["underpass", "lobby"]),
        ("skyline_168", "skyline", vec!["rooftop", "helipad"]),
    ] {
        let level = level_file(name);
        let tr = track(song).unwrap();
        assert_eq!(level.bpm, tr.bpm, "{name}");
        let chart = compile(&level, &builtin_lib()).unwrap();
        assert!(chart.total_beats <= tr.total_bars() as f64 * 4.0, "{name} runs past its song: {} beats", chart.total_beats);
        let path: Vec<String> = chart.rooms.iter().map(|r| r.name.clone()).collect();
        assert_eq!(path, rooms, "{name}");
        assert!(!chart.cues.is_empty(), "{name} has set pieces");
        let r = run_level(&level, Bot::human(22.0), SimConfig { seed: 7, ..SimConfig::default() });
        assert_eq!(r.report.downs, 0, "{name}: the human bot went down");
        assert!(r.report.stars > 3.5, "{name} stars {}", r.report.stars);
    }
}

#[test]
fn the_boss_is_a_chart_within_the_chart() {
    let level = level_file("skyline_168");
    let chart = compile(&level, &builtin_lib()).unwrap();
    // Three phases on the song's boss sections (1-based bars 55, 73, 97; the last ends at 113).
    let bars: Vec<(f64, f64)> = chart.phases.iter().map(|p| (p.start_beat / 4.0 + 1.0, p.end_beat / 4.0 + 1.0)).collect();
    assert_eq!(bars, vec![(55.0, 73.0), (73.0, 97.0), (97.0, 113.0)]);
    assert!(chart.phases[2].last && !chart.phases[0].last);
    // Each phase is its own segment: a lost phase restarts that phase only.
    let segs: Vec<u32> = chart.phases.iter().map(|p| p.segment).collect();
    assert_eq!(segs, vec![3, 4, 5]);
    assert!(chart.targets.iter().filter(|t| t.phase == Some(1)).all(|t| t.segment == 4));
    // The human bot clears all three; a bot that skips half its targets loses
    // phases, restarts them (downs == restarts, phase fails among them) and
    // its replay re-scores exactly.
    let good = run_level(&level, Bot::human(22.0), SimConfig { seed: 7, ..SimConfig::default() });
    assert_eq!((good.report.phases_cleared, good.report.phase_fails), (3, 0));
    assert!(good.report.score > 50_000);
    // Enemy fire cannot down this bot (a million HP), so every down is a lost
    // phase. Half its targets skipped against phases that clear at 45%: some
    // attempts fall short, the phase restarts, the run still ends on the boss kill.
    let mut loose = level.clone();
    if let Segment::Boss { phases, .. } = &mut loose.segments[3] {
        for p in phases.iter_mut() {
            p.clear = 0.45;
        }
    }
    let mut fails = 0;
    for seed in 1..=4 {
        let tank = Game::from_level(&loose, ClockConfig::default(), Windows::default(), Scoring { max_hp: 1_000_000, ..Scoring::default() }).unwrap();
        let bad = run_game(tank, Bot { skip_prob: 0.5, ..Bot::perfect() }, SimConfig { seed, ..SimConfig::default() });
        fails += bad.report.phase_fails;
        assert_eq!(bad.report.downs, bad.restarts, "seed {seed}");
        assert_eq!(bad.report.downs, bad.report.phase_fails, "seed {seed}: every down is a lost phase");
        assert_eq!(bad.report.phases_cleared, 3, "seed {seed}: the run ends when the last phase clears");
        assert_eq!(bad.replay.clone().unwrap().play().unwrap(), bad.report, "seed {seed}");
    }
    assert!(fails >= 1, "no phase was ever lost");
    // The fingerprint carries the phase rule: a looser clear ratio is another chart.
    let mut looser = level.clone();
    if let Segment::Boss { phases, .. } = &mut looser.segments[3] {
        phases[2].clear = 0.5;
    }
    assert_ne!(compile(&looser, &builtin_lib()).unwrap().fingerprint(), chart.fingerprint());
}

#[test]
fn cues_fire_on_their_bars_and_again_after_a_restart() {
    use downbeat_core::chart::Cue;
    use downbeat_core::GameEvent;
    // No inputs, no fail: the clock runs through the song and every cue fires once, on its bar.
    let level = level_file("night_market_132");
    let mut g = Game::from_level(&level, ClockConfig::default(), Windows::default(), Scoring { no_fail: true, ..Scoring::default() }).unwrap();
    g.start(-1.0);
    let spb = 60.0 / 132.0;
    let mut fired: Vec<(String, f64)> = Vec::new();
    while g.now_chart_time() < 70.0 * 4.0 * spb {
        let now = g.now_chart_time();
        let events: Vec<GameEvent> = g.update(1.0 / 120.0, None).collect();
        for e in events {
            if let GameEvent::Cue(i) = e {
                fired.push((g.chart().cues[i].kind.clone(), now));
            }
        }
    }
    let kinds: Vec<&str> = fired.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(kinds, vec!["lights", "lights", "train", "lights", "shake"]);
    let train = fired.iter().find(|(k, _)| k == "train").unwrap().1;
    assert!((train - 256.0 * spb).abs() < 0.02, "train at {train}, bar 65 is {}", 256.0 * spb);
    // With fail on and nobody shooting, the warmup's single encounter restarts
    // over and over; a cue inside it fires again on every attempt.
    let mut warm = warmup_level(128.0);
    warm.cues.push(Cue { bar: 4.0, kind: "lights".into(), room: String::new(), bars: 0.0, param: 0.0 });
    let mut g = Game::from_level(&warm, ClockConfig::default(), Windows::default(), Scoring::default()).unwrap();
    g.start(-1.0);
    let mut count = 0;
    for _ in 0..(120 * 90) {
        let events: Vec<GameEvent> = g.update(1.0 / 120.0, None).collect();
        count += events.iter().filter(|e| matches!(e, GameEvent::Cue(_))).count();
    }
    assert!(g.restarts() >= 2, "restarts {}", g.restarts());
    assert!(count >= 2 && count <= g.restarts() as usize + 1, "cue fired {count} times over {} restarts", g.restarts());
}
