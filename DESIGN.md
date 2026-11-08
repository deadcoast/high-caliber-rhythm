# highcal_rhythm — Design mechanisms

Working codename (mine, replace at will). Cel-shaded cyberpunk FPS rhythm game that is an aim trainer.
The game itself (identity, modes, phases) is in PLAN.md; this file holds the mechanisms under it.

## The drummer rule (godot/src/core/db.gd, kit.gd)

- The song's drum stem is the player's part. It plays through the lead-in so the pulse is given, mutes
  at the first target's spawn, and from then on every on-beat hit plays that target kind's kit sample
  (Perfect 0 dB, Great -3, Good -7). Off-beat and whiff play DEAD, a rim thud. A tracking target that
  finishes above 50% plays the cymbal at a gain set by its ratio. Miss the drums and the band plays
  without them; that is the whole feedback loop, heard.
- The click track follows the same rule: hats and accent are the band, the pulse layer is the drummer.

## Engine: Godot 4.7 + Rust core (GDExtension)

Merits, not effort: the game model (clock, charts, judge, scoring) lives in a dependency-free Rust crate so it
is deterministic, unit-tested without an engine, replayable (same inputs → same score, which leaderboards
need), and reusable in a browser chart editor via wasm. Godot owns rendering, input and audio output, and it
has a shader language and scene tooling suited to the cel/ink pipeline. Bevy would keep everything in Rust and
give rawer control of input latency, at the cost of building a toon/ink renderer, UI and audio stack from
parts; that cost is effort, named as effort, and it is the reason Bevy lost rather than any gameplay
limitation.

## The beat clock (core/src/clock.rs)

- Song time = seconds since stream start. Beat 0 = `song_offset_s`. Every other time is derived from BPM.
- The engine reports `playback_position + time_since_last_mix - output_latency` each frame. That value is
  quantised to the mixer chunk (~11.6 ms at 512/44.1k), so the conductor integrates frame time and pulls
  toward the audio value with gain 0.12; disagreement over 50 ms snaps (seek, loop, underrun). Measured in
  simulation: 1.2 ms RMS drift against a jittery chunked clock, 0 snaps.
- Ticks (sixteenths), beats and bars are emitted as crossings, so nothing is missed on a slow frame and
  nothing fires twice.
- Clicks are timestamped as `song_time_at_last_update + wall_clock_since`, taken before any scene work.
  Godot delivers input once per frame; at 240+ fps that is ±2 ms, below the Perfect window.
- Calibration: 8 bars of click, the player taps, the median distance-to-beat becomes `input_offset_ms`.
  This measures the whole pipeline on the player's machine (mouse polling, frame, audio output) in one number.

## BPM-space charts (core/src/chart.rs, patterns.rs)

- A pattern is a 4-beat phrase: events at fractional beats, target placement in yaw/pitch/distance from the
  lane origin, radius in metres, lead in beats, hold in beats for tracking.
- A level is pattern names × repeats + BPM. `compile` produces absolute targets. The same level at 90 and
  174 BPM is identical in beat space (tested) and 1.93× faster in seconds. A 1-beat lead is 667 ms at 90 and
  345 ms at 174: the difficulty curve comes from the tempo, not from re-authoring.
- `Scaling.radius_exp` is an author knob (default 0): negative exponent grows targets at fast tempo if a
  pattern needs it. My invention, not a rule.
- Song mapping (next stage): a song file + BPM + `song_offset_s` + a section map (bars → pattern sequence).
  Patterns are 4-beat phrases precisely so they snap to bars.

## Judgement (core/src/judge.rs)

- Windows in milliseconds: Perfect ±30, Great ±60, Good ±100. Beyond that a landed shot is EARLY/LATE: it
  consumes the target, counts as a spatial hit for aim stats, and as a timing miss for beat stats.
- Soft gating: the gun always fires. Off-beat is punished by score, never by refusing input. FPS players do
  not tolerate input denial; Metal: Hellsinger chose the same.
- Timing is judged against the target's own hit beat, not the nearest beat, so a shot at beat 7.4 is "late
  for 7", never "early for 8".
- A whiff reports distance to the nearest grid beat so the HUD can still show where the pulse was.
- Tracking targets are held, not clicked: one sample per sixteenth while the crosshair is on the target and
  the trigger is down; result is on/total.

## Replays (core/src/replay.rs)

- Every input is recorded with `at`, the chart time the judge had reached when the input was applied.
  Replay seeks to `at` before each input, so spawns, expiries and tracking state match the live run
  regardless of the live frame rate. A replayed run reproduces the live report exactly (tested at three
  tempos, JSON round trip included; `serde_json` runs with `float_roundtrip` for that).
- `fire` advances the judge to the click's own moment before judging. Verdicts never depend on where
  the frame boundary fell; a target is shootable until `hit + late_ms` (160), then expires.
- Runs are saved to `user://replays/<level>_<time>.json`; `--replay=path` re-scores one headless.

## Song mapping (core/src/analysis.rs, dbmap)

- `dbmap song.wav` finds tempo and beat 0: log-band spectral flux (low bands weighted ×2 because the
  beat lives in kick and bass), autocorrelation for the coarse tempo, then a joint period/phase comb
  search over the whole song. Synthetic mixes at 92/128/140/183.5 BPM come back within 0.05 BPM and
  4 ms of the truth (tested). Real songs still get checked by ear against the printed onset grid.
- The tool writes a level whose patterns fill the song's bars from a named set (warmup, flick, switch,
  track, micro). Patterns are 4-beat phrases so they snap to bars; a section map by song structure
  (intro/verse/chorus intensity) is the next authoring layer.

## Difficulty (core/src/rating.rs)

- Each shot's strain is Fitts' index of difficulty in bits, log2(travel / target diameter + 1), divided
  by the time available (the shorter of the gap from the previous target and the target's lead). That is
  the aim-trainer throughput demand of the shot. Tracking targets contribute path speed over angular
  radius. Stars = peak strain (top fifth) + 2 × density + 0.5 × tracking, anchored so the warmup at
  128 BPM is 3.0; the same chart rates 2.1 at 90 and 4.1 at 174.
- `dbmap --auto` picks each 2-bar phrase's pattern tier from the song's onset energy (rank among the
  song's phrases, smoothed over 4 bars), so intros and breakdowns get rest/micro and drops get 8ths and
  switches. Both numbers are mine; they tune against real songs and real players.

## Chart fingerprint and scores

- `Chart::fingerprint` hashes tempo and every target's beat, kind, placement and size. Replays carry it
  and refuse to re-score against a changed chart. Scores (`user://scores.cfg`) key bests by level name and
  fingerprint, so a pattern edit starts a fresh table instead of mixing runs on different targets.

## Stems (godot/src/core/db.gd)

- All audio plays through one `AudioStreamSynchronized`, so stems stay sample-locked and the clock
  reads one position. Combo tiers 0/10/25/50 open stems 0..3; any break drops to tier 0 and ducks the
  base stem 9 dB for 350 ms. The click track ships as three layers so the system is audible now.

## Scoring (core/src/score.rs)

- Shot = 100 × timing (1 / .75 / .5 / 0) × (0.6 + 0.4 × precision) × combo (1→2× by combo 50).
  Precision = 1 − ray-to-centre distance / radius, so a rim clip and a centre punch are both hits with
  different value; aim-trainer stats stay binary hits/shots alongside.
- Stats reported separately: aim accuracy, mean precision, on-beat %, timing quality, mean offset (ms),
  offset SD (ms), track %, per kind. The mean offset is the number a player uses to calibrate; the SD is the
  number that improves with practice.
- Grade from a composite (0.4 timing quality, 0.3 aim accuracy, 0.15 precision, 0.15 track).

## What is verified here, and what is not

- Verified in this environment: `cargo test` (31 tests: clock, chart, judge, score, replay, analyser,
  rating, fingerprint, full bot runs at 90/128/174), `dbsim` sweeps, `dbmap` on synthetic songs, the
  GDExtension builds and exports `gdext_rust_init`, every GDScript file parses (gdtoolkit) and every
  engine member, constant and call arity it uses exists in the Godot 4.7 API (`tools/gdcheck.py`).
- Not verified here: the Godot scene has not been executed; the sandbox refuses to run the engine
  binary. `tools/build.ps1 -Test -Smoke` is the first run on your machine: engine-side tests drive the
  binding, the click layers and every level, then the exact autoplay bot plays the warmup through the
  real input path. Any expired target or whiff there is a scene bug.

## Open knobs (my defaults; change by editing one number)

- Windows 30/60/100 ms. Combo cap 50 → 2×. Precision floor 0.6. Lead-in 8 beats. Pre-roll 2 s.
- Lane distance 12 m, target radii 0.3–0.6 m, FOV 80 vertical, 34 cm/360 at 800 DPI.
