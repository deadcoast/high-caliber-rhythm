# DOWNBEAT — Design mechanisms

Working codename (mine, replace at will). Cel-shaded cyberpunk FPS rhythm game that is an aim trainer.
The game itself (identity, modes, phases) is in PLAN.md; this file holds the mechanisms under it.

## Rooms, segments, enemies (core/src/chart.rs)

- A level is rooms plus a path of segments: traversal (a bar budget to move) and encounters (a room
  and its patterns). Rooms are boxes with a stand point; patterns are authored as angles from the
  stand, so one pattern plays in any room. The compiler lays rooms out along -Z and sizes each
  corridor to what the player can cover in the traversal's bars at 6 m/s with 25% slack: the level is
  sized to its song, and a faster song is a shorter level.
- Weak points are clamped between knee height and the ceiling, so a downward-authored angle lands on
  a low enemy rather than in the floor.
- Every target kind is an enemy type (drone, gunner, strider, swarm, heavy). The enemy's body chips and
  is never judged; the weak point is the hit object. An enemy that leaves its window alive fires:
  `Expired` is followed by `Damage` (by type) or `Down` when HP hits zero; HP returns +1 per on-beat
  kill. All of it is in the model, so replays carry damage and downs exactly.
- Doors out of a room open at the start of the next traversal; encounters begin on their bar whether
  the player has arrived or not.

## Section restart (core/src/lib.rs)

- When the first enemy of an encounter spawns, the session is checkpointed. A down restores that
  checkpoint (downs and damage taken are kept, HP refilled), resets the segment's targets to pending,
  rewinds the clock to one bar before the segment's first spawn and emits `Restart`; the engine seeks
  the song, clears live enemies and shows RETRY. The failed attempt leaves no trace in the stats.
- Replays record the moment of each down as an `Advance` input, so the replayed model reaches the
  same time, takes the same down and rewinds identically. Tested: a bot that skips a third of its
  targets restarts until it clears and its replay matches bit for bit. NF mod (`Scoring.no_fail`)
  keeps the HP reset without the restart.

## Mods, performance, integrity (core/src/mods.rs, rating.rs, integrity.rs)

- Mods compile a different chart (DT/HT via tempo, HR via radius) so the fingerprint differs and a
  modded replay only re-scores with its own mods; NF sets `no_fail`; HD is engine-side (no rings) and
  carried in the label. Score = raw × product of multipliers.
- `performance(stars, composite, combo_ratio, mods)`; `profile_rating` sorts best-per-chart pp and
  weights 0.95ⁱ. Both formulas are mine; the shape (difficulty squared, accuracy cubed, FC keeps 40%)
  is what makes a harder FC worth chasing over a safe clear.
- `integrity::inspect` runs after re-scoring: timing SD under 3 ms over 30+ hits, mean precision over
  0.995, >80% of shot gaps on an exact sixteenth grid, inputs out of time order. A perfect bot trips
  all three timing/aim checks; the human-error bot passes clean. Flags hold a score for review.

## The Downbeat volley (core/src/lib.rs, judge.rs)

- Charge: +1 per on-beat kill to 16; any expiry, whiff or off-beat hit zeroes it. With a full charge,
  `mark` accepts live non-track weak points; `release` is judged against the nearest bar line with the
  normal windows. On the bar: every marked target still live dies as a Perfect/Great/Good hit with the
  release timing (combo and pp accrue). Off the bar or with nothing marked: `VolleyMiss`, combo breaks,
  charge and marks gone. Marks and releases are replay inputs.

## Movement feel (godot/src/player/player.gd)

- Head bob is driven by beat phase while moving (lowest point on the beat), footsteps play on beats
  while moving, the dash recharges on each bar and extends 40% when started within ±90 ms of a beat,
  camera FOV punch and alternating roll size themselves by the verdict, damage shakes. The game
  clock is never scaled; everything is visual.

## The Range (core/src/patterns.rs drills, godot/src/core/stats.gd, src/ui/range_menu.gd)

- Seven drills (gridshot, eighths, switch, micro, strafe, fill, warmup), each a pass of patterns
  repeated to fill the chosen minutes at the chosen BPM in the range room. Every run records its
  composite (the grade formula) and per-kind numbers to `user://stats.json`. The Range shows per-drill
  means in the current 20-BPM band, the band rank from the last 20 runs, and a daily routine of the
  five weakest cells (unplayed cells first). Rank thresholds and band width are mine.

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

## Watching replays (core/src/lib.rs `advance_judge`, godot/src/core/db.gd `_feed_replay`)

- A replay is watched live: the game is rebuilt from the replay (level, mods, calibration), the song
  plays, and before each frame's update the driver applies every recorded input whose chart time falls
  inside the frame, advancing the judge alone to the input's recorded `at` first, exactly as
  `Replay::play` does. The song clock is never touched, so the picture stays on the audio and the model
  is exact. A frame boundary that crosses an expiry before a recorded Advance time downs the model on its
  own; the queued inputs up to that Advance are dropped as the dead attempt they are. Tested in both
  directions (live 240 fps watched at uneven 4–40 ms frames with hitches; live 30 fps watched at 240 fps).
- Two model fixes fell out of it: the Advance input carries the judge's time at the down, not the
  conductor's, and a click or release whose own judge-advance found the downing expiry is swallowed (the
  player was down at that instant) rather than recorded against the rewound section.
- The engine records the camera once per tick (`view` key appended after the core's replay text, which is
  kept byte for byte) so the watch follows where the run was played from; older replays stand at the
  room's stand and snap onto each shot. Watching records no score, no stats, no replay.

## Results screen (godot/src/ui/results_screen.gd)

- Grade stamp, score, pp, combo, FC/PFC, the aim-trainer line (hits, aim, precision, on-beat, mean
  offset, SD, damage, downs), judgement histogram, timing histogram in 10 ms bins, timing scatter by beat,
  per-enemy table, the chart's leaderboard with this run marked and integrity flags, and the actions:
  retry, watch the replay, Range. The data is the report the model wrote; the drawing is greybox (HUD-06).

## Any Song (godot/src/core/db.gd, src/ui/any_song.gd, song_confirm.gd)

- Pick a file. The core decodes it (symphonia: wav, flac, ogg, mp3, m4a) and measures tempo and beat 0 on
  a worker thread against a second model instance, so the main thread never blocks and the game instance
  is never touched off-thread (gdext `experimental-threads`). The decoded PCM becomes the run's stream, so
  no engine importer is involved and any format the core reads plays.
- The confirm screen plays the song with a metronome rendered onto the proposed grid in the same
  synchronised stream (sample-locked), shows the onset grid over four bars and the energy per bar, and
  takes nudges: the analysis is cached so a nudge regrids instantly. Enter builds the energy-mapped level
  (`auto_level`) into `user://songs/` and plays it through the normal level path: restarts, scores,
  replays, leaderboards all apply; the level keeps the song's absolute path.

## Menu (godot/src/ui/menu.gd)

- Level select lists the builtins at a chosen BPM and every level file (shipped and Any Song), each rated
  under the current mod set with the leaderboard best for that mod set (`rate_level_json_mods`: DT/HT
  change the compiled tempo, HR the radii, so the fingerprint and stars shown are the ones a run would
  be keyed and scored by). Settings edits every value in `user://downbeat.cfg` live; consumers listen to
  `Settings.changed`. Pause stops the tree (audio pauses with it; the conductor resumes on the same
  audio position). Greybox drawing, final data and flow (UI-01).

## The boss (core/src/chart.rs `Segment::Boss`, judge.rs `PhaseEnd`, godot/src/world/boss.gd)

- A boss segment is a room and a list of phases; each phase compiles to a segment of its own, so the
  checkpoint-and-restart machinery that sections already have gives every phase its own restart point
  for free. Phases get `PhaseInfo` in the chart (and a line in the fingerprint: a looser clear ratio is a
  different chart).
- The judge marks every target killed by a hit, a volley or a held track; when the clock passes a
  phase's end plus the late window it closes the phase once and emits `PhaseEnd` with kills, total and
  whether kills reached `ceil(clear × total)`. The session answers a lost phase with `Down` (the boss's
  attack lands whatever the HP), which restarts that phase; a cleared phase pays a flat bonus, the last
  one the boss bonus. Tested: the human bot clears all three phases of Skyline; a bot skipping half its
  targets against 45% phases loses some, restarts them, ends on the kill, and its replay re-scores exactly.
- A checkpoint is taken at a segment's first spawn, which can land before the previous segment has
  resolved (its last window, a phase's close), so results that belong to an earlier segment are applied
  to the later checkpoints too. Found by the boss tests: a lost final phase used to restore a checkpoint
  that had never seen phase two clear.
- In the engine the boss is one body in the boss room; the weak points of every phase hang off it as
  tethered parts with no body of their own. The reveal cue raises it, a cleared phase drops two plates,
  a lost phase is the red attack flash and a camera shake, the last clear is the kill.

## Set pieces (core/src/chart.rs `Cue`, godot/src/world/level.gd)

- A cue is a bar, a kind, a room, a length in bars and a parameter. The compiler times it; the game emits
  `GameEvent::Cue` when the clock reaches it and again after a restart that rewinds past it (tested).
  Cues never touch judging and stay out of the fingerprint. The engine plays them: a train through the
  platform's trench, the key-light scheme of the room strips and the ambient, the underpass water rising,
  the boss reveal, a camera shake.

## Campaign (godot/src/core/campaign.gd)

- Act one is a list of level files in order. A level is cleared with grade C or better (call: one number)
  and unlocks the next; the act is clear when its last level is. Best grade, score, pp, plays and cleared
  are kept per level in `user://campaign.json`; the menu shows the act with locks and bests above level
  select. No mid-level saves: a level is three minutes and every section is a restart point already.

## Online (server/, godot/src/core/online.gd, src/world/ghost.gd)

- A score is a replay and nothing else. The server parses it, re-scores it with the same crate
  (`Replay::play`), checks the chart fingerprint against the level the replay carries, runs
  `integrity::inspect`, and stores the server's numbers. The client's own score is never read.
- Ranked means the server computed that chart itself: every shipped level file and the four builtins at
  every whole BPM from 60 to 220, under every chart-changing mod combination (DT/HT × HR; HD and NF do
  not change the chart). Anything else, an Any Song chart or an edited level, still gets a board among
  the people who play that exact fingerprint, marked unranked. Boards are one entry per player, clean
  before flagged, then by score; an abandoned run (the replay ends before the chart) is stored unranked.
- The daily is deterministic from the UTC date: the date is hashed, the pool is the shipped levels and the
  three builtin action levels at 120–180 BPM, a third of days carry HR and a sixth DT; the client gets the
  level JSON and mods, plays it, and posts to the daily board, which accepts today's and yesterday's chart
  for runs that cross midnight. No daily is ever stored; it can always be recomputed.
- Ghosts: the #1 replay of a board is loaded into a second model instance and fed in lockstep with the
  live run by the same horizon rule the watcher uses (`GhostFeeder`), locked to the song's clock while
  the ghost is on the same timeline; a restart of the ghost's own makes it fall behind and it runs open
  loop from there, which the HUD says. Its hits flash a ring on the enemy it killed, its aim is drawn
  from the recorded view track, its score runs against yours. Tested headless: a ghost fed at a
  different frame rate than the live run ends on the replay's own report.
- Verified here: the server's API in process (register, re-score equal to the client's report, ranks,
  a perfect bot flagged and ranked under a human, a tampered level refused, a custom chart unranked,
  an abandoned run unranked, replay download re-scoring exactly, the daily served, played and boarded)
  and over TCP with the binary.

## Act one tracks (core/src/song.rs, godot/levels/*.json)

- Three plans over a progression and a motif each; Night Market below, Undertow at 150 with a half-time
  kick (a pickup into 3) and the melody on the fifth and flat seventh, Skyline at 168 with eighth-note
  runs and three boss sections with a build between the second and the third. The analyser reads all
  three grids back (the syncopated kick pulls Undertow's phase by about 20 ms, which is what the confirm
  screen's nudge is for; the levels state their offsets).

## Night Market (core/src/song.rs, godot/levels/night_market_132.json)

- The content pipeline needs a song before a commissioned one exists, so the core renders one: a section
  plan (intro, build, verse, chorus, break, build, drop, outro; 88 bars) over an A-minor progression,
  with a two-bar lead motif and its answer, four-on-the-floor kicks with a sixteenth roll on the last bar
  of each build, and a dotted-eighth delay on the lead. Four stems rendered deterministically in 1.2 s,
  normalised, beat 0 at the first sample.
- The chart was written in the editor's level format against that plan: encounters start on the section
  bars (asserted in a test), two inline patterns put the chorus flicks on the melody's own eighths, the
  build rolls are micro steps on the kick roll, the drop opens with the heavy. The analyser reads the mix
  back at 132.00 BPM / beat 0 within 2 ms, and the human bot clears it without a down. The stems are
  SCAFFOLD AUDIO; the plan, the level and the stem tiers are the product.

## Chart editor (core/src/wasm.rs, web/)

- The core builds for wasm32 with a hand-written C ABI (JSON requests, f32 samples for analysis; no
  bindgen). The Svelte editor compiles the level on every edit through the same compiler and rater the
  game uses, so stars, fingerprint and target placement match the engine exactly; the dbsim bot runs in
  the browser to prove a chart is clearable. Charts are authored against the song's measured grid with a
  WebAudio click on the beats and the kit on the targets; export is the engine's level JSON.

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

- Verified in this environment: `cargo test` (54 tests: clock, chart, judge, score, replay, watching at
  uneven frame rates, analyser, rating, fingerprint, full bot runs at 90/128/174, the wasm ABI), `dbsim`
  sweeps, `dbmap` on synthetic songs, the GDExtension builds and exports `gdext_rust_init`, every
  GDScript file parses (gdtoolkit) and every engine member, constant and call arity it uses exists in the
  Godot 4.7 API (`tools/gdcheck.py`); the wasm core runs under Node (`web/scripts/check-core.mjs`) and the
  built editor was driven in headless Chromium (load, edit, drag, simulate, export, analyse a song, auto
  chart, play) with no console errors.
- Not verified here: the Godot scene has not been executed; the sandbox refuses to run the engine
  binary. `tools/build.ps1 -Test -Smoke` is the first run on your machine: engine-side tests drive the
  binding, the click layers and every level, then the exact autoplay bot plays the warmup through the
  real input path. Any expired target or whiff there is a scene bug.

## Open knobs (my defaults; change by editing one number)

- Windows 30/60/100 ms. Combo cap 50 → 2×. Precision floor 0.6. Lead-in 8 beats. Pre-roll 2 s.
- Lane distance 12 m, target radii 0.3–0.6 m, FOV 80 vertical, 34 cm/360 at 800 DPI.
