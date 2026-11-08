          # DOWNBEAT

Cel-shaded cyberpunk FPS rhythm game / aim trainer. Greybox build: the backend is real, every visual is a
placeholder tracked in `ASSETS.register.md`.

```
core/     Rust game model (clock, BPM-space charts, judge, scoring, replays, analyser, tracks, simulator) + Godot bindings + wasm ABI
godot/    Godot 4.7 project: rooms, player, enemies, the boss, HUD, results, menu, Range, Any Song, replays, ghosts, levels
web/      chart editor: Svelte 5 over the wasm core (song analysis, path, patterns, boss phases, cues, timeline, rating, bot)
server/   online: replay re-scoring, leaderboards, the daily, ghosts (axum + SQLite, one binary)
art/      briefs and reference sheets for the art pass (the register in ASSETS.register.md is the schedule)
tools/    build.ps1 / build.sh: one command, builds the extension and stages it; -Web the editor, -Server the server
```

## Run

Requires Rust (stable) and Godot 4.7.x. No Unity, no Unreal.

```powershell
.\tools\build.ps1 -Test -Smoke       # Windows: build core, stage dll, engine tests, headless autoplay run
.\tools\build.ps1 -Run -Bpm 128      # build, then launch the warmup
```
```bash
tools/build.sh --test --smoke        # Linux/macOS
tools/build.sh --run --bpm=174
```

With no flags the game opens on the menu: act one (Night Market, Undertow, Skyline; a level unlocks when
the one before it is cleared with a C), then level select (builtins at any BPM, every shipped level, every
Any Song level; stars and best under the chosen mods), settings (`S`: sensitivity, DPI, offset with `C` to
calibrate, FOV, frame cap, post-FX, volume, cue tuning), `Tab` the Range, `O` the Any Song picker. `Esc`
pauses a run (resume, restart, level select, settings, Range, quit). Run flags after `--` start a run
directly: `--bpm=N`, `--range=<drill> --minutes=M`, `--warmup` / `--fill`, `--level=res://levels/x.json`,
`--song=<file>` (Any Song, no confirm), `--watch=user://replays/x.json` (watch a run), `--autoplay` (the bot
plays), `--smoke` (print the report and quit), `--replay=user://replays/x.json` (re-score a saved run),
`--list-levels` (every level with rating and best).

`python3 tools/gdcheck.py` checks every GDScript member, constant and call arity against Godot's
`extension_api.json` without launching the engine (uses `godot --dump-extension-api` output or the
copy in the gdext dependency cache).

In game: mouse look (34 cm/360 @ 800 DPI, in settings), LMB tap = shot / hold = beam, RMB hold = mark for
the Downbeat, release on the bar; WASD, Shift sprint, Ctrl slide (while sprinting), Space jump (mantles
low ledges), `Q` beat dash, `Tab` Range (1–5 toggle DT HT HR HD NF), `O` Any Song, `R` restart, `C` calibrate,
`Esc` mouse. Flags: `--hard` (Street Hard), `--mods=DTHR`.

After a run the results screen shows the grade, score, pp, judgement histogram, timing histogram and
scatter, per-enemy table and this chart's leaderboard; `Enter` watches the replay (the camera follows the
recorded run; `R` then plays the level yourself), `R` retries, `Tab` opens the Range, `Esc` closes.

Any Song (`O`): pick a wav/flac/ogg/mp3/m4a, the core measures tempo and beat 0 on a worker thread, the
confirm screen plays the song with a click on the grid (Left/Right nudge beat 0 by 10 ms, Shift 1 ms,
Up/Down halve/double the BPM, `[`/`]` ±0.5, Space skips 8 bars), `Enter` builds the energy-mapped chart into
`user://songs/` and plays it. Every song level restarts, scores and replays like a shipped level.

## Verify the model without the engine

```
cd core
cargo test                                  # 54 tests (core); cd ../server && cargo test for the API (10)
cargo run --bin dbsim -- --sweep            # bot plays the warmup at 90 / 128 / 174 BPM
cargo run --bin dbsim -- --bot perfect --bpm 160
cargo run --bin dbsim -- --street --bpm 140 # the act-one level, human bot
cargo run --bin dbsim -- --level-json 140 --street > ../godot/levels/street_140.json
cargo run --release --bin dbmap -- song.wav --set flick --audio res://audio/song.wav --out ../godot/levels
cargo run --release --bin dbmap -- song.wav --auto        # patterns chosen bar by bar from the song's energy
cargo run --release --bin dbmap -- --synth 140 0.42 30    # analyser self-test on a synthetic track
cargo run --release --bin dbmap -- --synth 128 0.2 60 6 --auto   # ... with a 6-bar quiet intro
```

Scores: top-10 leaderboard per chart fingerprint and mod set in `user://scores.json` (pp, grade, FC/PFC,
replay path, integrity flags); profile rating = osu-weighted best pp per chart.
Stats: every run in `user://stats.json`; the Range shows per-drill means in the current BPM band, the band rank
(Iron to Diamond from the last 20 runs) and today's routine (the five weakest cells). Cue tuning lives in
`user://downbeat.cfg` under `[cue]`: `ring_start` (ring size at spawn, × radius), `pop_ms`, `telegraph`.

## Chart editor

`tools/build.ps1 -Web` (or `cd web && npm install && npm run core && npm run build`) builds the editor into
`web/dist`; `npm run dev` serves it live. Song in, grid by ear, path and patterns by hand or from the song's
energy, timeline, rating and bot run on every edit, level JSON out. See `web/README.md`.

## Act one

Three levels, three tracks, one city at night. The tracks are produced by the core (`core/src/song.rs`,
`dbsong`): section plans over a progression with a motif each, rendered to four stems (`base` kick/bass/pad,
`perc` hats/clap, `keys` stabs, `lead` melody/snare) that open at combo 0/10/25/50. The build renders them
into `godot/audio/` when missing. SCAFFOLD AUDIO: commissioned tracks replace the stems; the plans are the
briefs. Every chart is in the editor's format, encounters on the song's sections, cleared by the human bot
(tested).

| Level | Track | Rooms | Set pieces |
|---|---|---|---|
| 1 NIGHT MARKET 132 | A minor, 88 bars | street → atrium → platform | lights on the chorus, a train through the platform on the drop |
| 2 UNDERTOW 150 | D minor, half-time kick, 88 bars | underpass → lobby | the water rises through the build, lights on the chorus and the drop |
| 3 SKYLINE 168 | E minor, 120 bars, three boss sections | rooftop → helipad | the crane rises on the break; THE CRANE, three phases: gun pods, searchlights, the core |

The boss is a chart within the chart (`"type": "boss"` segment with `phases`): every phase is a section
list on its part of the song with a `clear` ratio; when the phase closes, kills below it are the boss's
attack (a down; the phase restarts), at or above it the phase clears (+500) and the last one is the kill
(+2000). The HUD shows the boss bar, the phase's kills against its clear line and the bars left.

Set pieces are `cues` in the level JSON (bar, kind, room, bars, param), timed by the model and played by
the engine: `train`, `lights` (0 night, 1 magenta, 2 cyan, 3 red), `flood`, `reveal`, `shake`.

Progress is saved to `user://campaign.json` (best grade, score, pp, plays, cleared per level).

## Online

`server/` is the online layer: set its URL in settings (`online/url`, Enter on the row) and every finished
run's replay is posted. The server re-scores it with the same core, inspects it, ranks it, and the results
screen shows your world rank and the world top five next to the local board; `G` there fetches the #1
replay and races it as a ghost (a hollow crosshair where they aimed, a white ring on every enemy they
killed, their score against yours). The menu's top row is the DAILY: one chart a day for everyone, drawn
from the ranked set by the date, with its own board. Range runs stay local. See `server/README.md`.

## Songs

`dbmap song.wav` prints tempo, beat 0 and an onset grid, and writes a level that fills the song's bars.
Put the audio under `godot/audio/` and pass `--audio res://audio/<file>` (or list `stems` in the JSON:
they play sample-locked and open by combo tier). Then `.\tools\build.ps1 -Run` with
`--level=res://levels/<name>.json`. Settings persist in `user://downbeat.cfg`; replays in `user://replays/`.

## Levels

`godot/levels/*.json`. `bpm`, `lead_in` (beats), `rooms` (name, size, stand, yaw_deg, blocks; origin
optional, laid out by the compiler), `segments` (`traversal {bars}` / `encounter {room, sections}`), or the
shorthand `sections` for a single Range encounter; optional inline `patterns`; `audio` (empty = click),
`stems` (combo-tiered), `song_offset_s`, `kit` (kind label → hit-sound path; empty = synthesised).
Builtin patterns: `flick_lr`, `flick_quad`, `flick_8ths`, `switch_pair`, `micro_walk`, `micro_zigzag`,
`track_sweep`, `track_arc`, `rush_fill`, `rest`. Builtin levels: `street` (act one, level one), `warmup`
(Range reference, rating anchor), `fill` (Range, every kind).

The plan: `PLAN.md`. Design mechanisms: `DESIGN.md`. Art schedule: `ASSETS.register.md`.
