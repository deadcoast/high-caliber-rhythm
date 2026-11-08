# DOWNBEAT — Game and production plan

Working codename, mine. One thread, seven phases. Everything below that the brief did not state is
marked as a call; calls are reversible and noted, not questions.

## 1. The game

**You are the drummer, and the drum kit is a gun.** The music's percussion is not pre-recorded: it is
the player. Every chart target is a drum hit. Shoot it on the beat and that hit sounds in the mix;
miss it and the song plays without it. A perfect run is the song as written; a bad run is the band
without a drummer. Timing is heard, not just scored. *(Call: this is the identity. Hellsinger raises
intensity with accuracy; BPM gates your trigger; nothing on the market makes the player the
percussionist.)*

Three audiences, one product:

| Audience | What they are buying | Where it lives |
|---|---|---|
| Rhythm players | Songs as drum charts, performance, grades, FCs | Sets |
| Aim trainers | Scenarios in beat-space at any BPM, stats, routines, rank | Trainer |
| FPS players | A shooter that feels better than their main, a crowd | Both, through the feel |

### Target kinds are drum voices

| Kind | Voice | Chart role | Aim skill | Silhouette brief (register TGT-*) |
|---|---|---|---|---|
| FLICK | Kick | Downbeats, quarter notes | Click timing | Heavy core, low mass |
| SWITCH | Snare | 2 and 4, pairs alternating | Target switching | Twin heads, one body |
| MICRO | Hi-hat | 8ths, small steps | Micro-adjust | Small, bright, chained |
| TRACK | Cymbal / riser | Swells over bars | Smooth tracking | Long, trailing |
| RUSH | Tom fill | 16th bursts before a drop | Speed | Stacked, short-lived |

RUSH is new to the pattern library (added in P1). Each kind has one hit sound per song key and one
kind-specific death. Players learn the kit the way a drummer does: by sound before sight.

### Modes

1. **Sets** — the campaign. A night across venues; each song is a hand-authored chart. Grades unlock
   the next song, venue lighting and crosshair cosmetics. No gameplay power in unlocks.
2. **Trainer** — the aim-trainer product. Scenario library (the pattern sets) at any BPM with the
   click kit, stats history per kind (aim accuracy, precision, offset mean/SD), daily routine, rank
   per BPM band ("Plat at 140"). BPM is the difficulty dial, and that dial is the pitch to the
   aim-training community: your aim measured against tempo.
3. **Any Song** — drop a WAV/OGG. In-engine analysis (already in the core) makes the grid and an
   energy-mapped chart. The Beat Aim / Audiosurf hook, done with a real judge.
4. **Daily** — one fixed chart a day, leaderboard, replays validated server-side.
5. **Ghosts** — race a replay (yours or a friend's) rendered as a ghost crosshair. Live versus later.

### Space and body

The arena is a stage, not a map. One lane, 12 m deep, the player on a pad with free look and short
strafe; movement exists for feel and never for routing. Venues are the content axis of Sets: basement
club, rooftop, metro platform, flooded underpass (register ENV-*). Crowd and lights react to stem
tier. *(Call: no traversal. An FPS player's body is in the camera and the gun; the lane keeps every
target in the aim-trainer cone.)*

### Weapon

One hitscan weapon, "the Kit". Fire on click, hold for tracking. Viewmodel animation keyed to the
verdict. No reloads, no ammo, no weapon swap. *(Call: a second weapon would fight the drummer metaphor
and the trainer's purity. Revisit after P4 only if Sets need it for variety.)*

### Scoring and progression

Two-axis score as built (timing × precision × combo). Grade S–F. Per-kind stats. Combo tiers open
stems. FC (full combo) and PFC (all Perfect) are the badges. Rank in Trainer is per BPM band from a
rolling window of runs, aim-trainer style.

### Art direction

Borderlands ink and flat bands, Cyberpunk 2077 palette and materials. Two shading rules are already
in the pipeline: banded toon ramp with hard rim; ink line from a depth+normal edge pass. Targets are
the icons and ship first. Everything in ASSETS.register.md is VOID until produced; the register is the
art schedule.

### Music

Commissioned/licensed tracks delivered as stems (drums, bass, lead, pad), 120–180 BPM, electronic,
industrial, metal. The drum stem is the player's; the song ships with it muted and each chart target
carries its hit sample. `dbmap` produces the grid and the baseline chart; a human charts the drum part.

## 2. Architecture (built)

- `core/` Rust: conductor (audio-locked clock), BPM-space charts, judge (ms windows, soft gating,
  frame-independent), scoring, replays with chart fingerprint, song analyser, difficulty rating, bot
  simulator. 31 tests. Compiles to the GDExtension and later to wasm for the web editor.
- `godot/` 4.7: driver (`DB`), stems via `AudioStreamSynchronized`, settings, scores, arena, player,
  targets, weapon hooks, HUD, post-FX hook, autoplay bot, headless engine tests.
- `tools/`: one-command build/test/smoke, `dbmap` song mapper, `gdcheck` API checker.

## 3. Phases

Each phase is a slice of every layer (model, engine, content, verification), not a department. A
phase ends when its exit criterion runs green, not when its list is typed.

### P0 — Model and greybox (done)

### P1 — Feel

The drummer rule, the kit sounds, and the first run on real hardware.

- Drum-stem rule: a song's drum stem is muted; each on-beat hit plays the target's hit sample at the
  verdict's gain (Perfect full, Good soft). Off-beat and whiff play the kit's "dead" sound. Click
  track: the pulse layer becomes the player's.
- RUSH kind, 16th-note pattern, rating and HUD support.
- Hit SFX tiers (AUD-02) as placeholder synthesised hits so the rule is audible before production
  sound exists.
- First hardware run: `-Test -Smoke`, then play. Latency budget measured: mouse-to-judge under 8 ms at
  240 fps, calibration offset stable to ±5 ms across three runs.
- Cue tuning on hardware: approach ring timing, target sizes per kind, judgment pop timing.
- Exit: PFC the warmup at 128 by hand with the click kit, calibration within ±5 ms, no judge surprises.

### P2 — Trainer

- Scenario library: every pattern set as a scenario, BPM dial 80–200, duration 1–5 min, click kit.
- Stats store (`user://stats.sqlite`, via Godot's SQLite-free path: a JSON log per run rolled into
  per-kind aggregates): aim accuracy, precision, offset mean/SD, by kind and BPM; rolling 7-day view.
- Daily routine: five scenarios chosen from the player's weakest kind/BPM cells.
- Rank per BPM band from the last 20 runs.
- Results screen as a real screen: grade, histogram, scatter, per-kind table, replay button (HUD-06).
- Settings and level select screens with real data (UI-01, UI-02) in greybox UI.
- Exit: a 20-minute trainer session end to end with no debug keys, stats persisted, ranks shown.

### P3 — Content pipeline

- Any Song in-engine: file picker → `analyze_wav` → grid → `--auto` chart → play; tempo/offset
  confirmation screen with the onset grid and a tap-to-confirm.
- Chart editor on the web: Rust core to wasm, Svelte front end; timeline in beats, pattern palette,
  per-target drag in yaw/pitch, playback against the song, export level JSON. *(Call: web, not
  in-engine. Charting is a desktop-browser task and the core is already engine-free.)*
- Three commissioned tracks with stems, charted by hand through the editor. First real music.
- Exit: one commissioned song charted in the editor, playing in Godot with the drum-stem rule, PFC-able.

### P4 — Sets

- Venue system: venue scene = lane + lights + crowd + skyline, data-driven lighting cues from stem
  tier and beat events. Two venues greyboxed.
- Set structure: night of 4–5 songs per venue, unlock by grade, cosmetics catalogue.
- Ten songs charted.
- Exit: a full night playable start to finish with progression saved.

### P5 — Art production

The register, in order, each row produced → imported → placeholder deleted → screenshot attached:
targets (TGT), ring and VFX, weapon (WPN), HUD type and crosshair pack (HUD), ink pass and ramps
(SHD), venues (ENV), post (SHD-03), sky (SHD-04). Generative models for concept spread and texture
bases; every shipped asset is hand-finished. Runs in parallel with P4 once targets are locked.

- Exit: no VOID rows in the register for Sets' first venue and the trainer.

### P6 — Online

- Leaderboards: score + replay upload, server re-scores with the core (same crate, headless), rejects
  fingerprint mismatches. Daily chart service. Ghost download.
- Exit: daily live for a closed beta of 50, zero unvalidated scores on the board.

### P7 — Release

- Steam page, trailer cut from in-engine capture, Next Fest demo (Trainer + two songs + Any Song).
- Twitch: channel-points pick the next song's BPM; chat ghost. *(Call: this is the FPS-audience
  acquisition channel; streamers are the distribution.)*
- Launch: Sets with 15 songs, Trainer, Any Song, Daily. Post-launch: editor charts shared via
  Workshop, versus.

## 4. Verification, standing

- Core: `cargo test`, `dbsim --sweep`, `dbmap --synth` self-tests. Zero warnings.
- Engine: `gdcheck` (API), `run_tests.gd` (binding, levels, audio layers), autoplay smoke (exact bot).
- Hardware: calibration stability, latency budget, PFC-by-hand of the reference chart per release.
- Art: register rule — a row leaves VOID only with the placeholder deleted and a screenshot attached.

## 5. Where we are

P1 starts now. Next build steps, in order: RUSH kind and pattern; drum-stem rule in `DB` with
synthesised kit hits; hit-sound tiers; `-Test -Smoke` on your machine and the fixes that follow;
cue tuning from that run.
