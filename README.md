# highcal_rhythm

Cel-shaded cyberpunk FPS rhythm game / aim trainer. Greybox build: the backend is real, every visual is a
placeholder tracked in `ASSETS.register.md`.

```
core/     Rust game model (clock, BPM-space charts, judge, scoring, simulator) + Godot bindings
godot/    Godot 4.7 project: arena, player, targets, HUD, shaders, levels
tools/    build.ps1 / build.sh: one command, builds the extension and stages it
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

Run flags after `--`: `--bpm=N`, `--fill` (every kind incl. RUSH), `--level=res://levels/x.json`, `--autoplay` (the bot plays),
`--smoke` (print the report and quit), `--replay=user://replays/x.json` (re-score a saved run),
`--list-levels` (every level with rating and best).

`python3 tools/gdcheck.py` checks every GDScript member, constant and call arity against Godot's
`extension_api.json` without launching the engine (uses `godot --dump-extension-api` output or the
copy in the gdext dependency cache).

In game: mouse look (34 cm/360 @ 800 DPI, edit in `player.gd`), LMB fire / hold to track, WASD, Space,
`R` restart, `C` calibrate (tap 16 beats, offset is set from the median), `Esc` release mouse.

## Verify the model without the engine

```
cd core
cargo test                                  # 27 tests
cargo run --bin dbsim -- --sweep            # bot plays the warmup at 90 / 128 / 174 BPM
cargo run --bin dbsim -- --bot perfect --bpm 160
cargo run --bin dbsim -- --level-json 140 > ../godot/levels/warmup_140.json
cargo run --release --bin dbmap -- song.wav --set flick --audio res://audio/song.wav --out ../godot/levels
cargo run --release --bin dbmap -- song.wav --auto        # patterns chosen bar by bar from the song's energy
cargo run --release --bin dbmap -- --synth 140 0.42 30    # analyser self-test on a synthetic track
cargo run --release --bin dbmap -- --synth 128 0.2 60 6 --auto   # ... with a 6-bar quiet intro
```

Scores: best run per level and chart fingerprint in `user://scores.cfg` (replay path included).

## Songs

`dbmap song.wav` prints tempo, beat 0 and an onset grid, and writes a level that fills the song's bars.
Put the audio under `godot/audio/` and pass `--audio res://audio/<file>` (or list `stems` in the JSON:
they play sample-locked and open by combo tier). Then `.\tools\build.ps1 -Run` with
`--level=res://levels/<name>.json`. Settings persist in `user://highcal_rhythm.cfg`; replays in `user://replays/`.

## Levels

`godot/levels/*.json`. `bpm`, `lead_in` (beats), `sections` of builtin pattern names × repeats, optional
inline `patterns`, `audio` (stream path; empty = procedural click), `stems` (paths, combo-tiered),
`song_offset_s` (seconds to beat 0 in the file).
`drum_stem` (index into `stems`; the player's part, muted once targets start), `kit` (kind label →
sample path; empty = synthesised kit).
Builtin patterns: `flick_lr`, `flick_quad`, `flick_8ths`, `switch_pair`, `micro_walk`, `micro_zigzag`,
`track_sweep`, `track_arc`, `rush_fill`, `rest`. Builtin levels: `warmup` (reference, rating anchor), `fill`.

The plan: `PLAN.md`. Design mechanisms: `DESIGN.md`. Art schedule: `ASSETS.register.md`.
