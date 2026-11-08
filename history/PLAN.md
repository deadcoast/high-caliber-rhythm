# DOWNBEAT — Game and production plan

Working codename, mine. One thread, phases inside it. Where the brief does not decide, the call is
marked as a call and is reversible.

## 1. The game

A linear first-person shooter. Cel-shaded cyberpunk city, levels you move through, enemies that shoot
back, a campaign. The whole shooter is built on the beat and on the art:

- **osu** gives the hit vocabulary. Every enemy carries a weak point with an approach ring that closes
  on its beat. Kill it on the beat. Enemies that move are sliders (track them under fire). Heavies are
  spinners (hit sequences on sixteenths). Songs have beatmaps, difficulties, mods, accuracy, FC.
- **Aim Lab** gives the encounter vocabulary and the measurement. Encounters are built from the aim
  scenarios players already train (gridshot, spidershot, switch, strafe-track, microshot, reflex) and the
  game records what Aim Lab records, per scenario, across the whole campaign: accuracy, precision,
  reaction, tracking, overshoot. Playing the campaign is training; the Range is the same encounters
  as pure drills at any BPM.
- **Linear FPS** gives the body and the place: movement that feels good (sprint, slide, jump, mantle),
  rooms with cover and height, traversal between fights, set pieces, a weapon that kicks, enemies with
  silhouettes, a city.

The song drives the level. A level is sized to its song: traversal segments have a bar budget, doors
open on downbeats, encounters start on the bar whether you are in position or not, and the finale
lands on the drop. Charts live in beat-space, so the same level plays at any tempo: a new song maps
onto the same rooms and the same patterns by its structure.

### Enemies are hit objects

| Enemy | Hit object | Aim scenario | Behaviour | Fails if unkilled |
|---|---|---|---|---|
| Drone | Circle | Gridshot, spidershot | Pops to an anchor, exposes core on its beat, leaves | Shoots you as it leaves |
| Gunner | Circle pair | Switch | Two peek alternately from cover on 8ths | Volley on the bar |
| Swarm bot | Small circle chain | Microshot | Small, many, stepped positions | Chip damage each |
| Strider | Slider | Strafe-track | Crosses the room on a path over bars; beam held on its core | Burst at path end |
| Heavy | Spinner | Reflex / rush | Eight plates in sequence on sixteenths before its charge | Charge: big damage |
| Boss | Chart within the chart | All | Phases keyed to song sections | Encounter fail |

On-beat weak-point hit kills (Perfect/Great/Good scale score and the kill VFX). Off-beat weak-point hit
kills late: no score, combo breaks, the enemy still gets its shot off if the window passed. Body hits
chip and do not kill. Whiffs break combo. Soft gating throughout: the trigger always fires.

Enemy fire is on the beat as well: an enemy that survives its window fires at its expiry. Damage by
type. HP 100, +1 per on-beat kill. HP 0 = down: section restarts from its first bar with the song
rewound (P2); until then the run ends with a fail report.

### Movement

Walk, sprint, jump, slide, mantle. Movement is free; it never has to be on beat. *(Call: Hellsinger and
BPM put movement on the beat and FPS players read that as input denial. Our beat lives in the shot and
in the world.)* Optional later: dash landing on a downbeat refunds stamina.

### Weapon

One primary with two fire modes, "the Kit": tap = hitscan shot (circles), hold = beam (sliders).
Heavy plates take taps. *(Call: one weapon in P1–P3 keeps the aim-trainer measurement clean. A second
weapon, shotgun-class for rush sequences, is a P4 decision.)*

### Modes

1. **Campaign** — linear levels across the city, one song each, hand-charted. Grades unlock the next.
2. **Range** — the Aim Lab product. Every encounter pattern as a drill at any BPM with the click kit;
   stats history per scenario; daily routine from the weakest cells; rank per BPM band.
3. **Any Song** — drop a file: in-engine analysis, energy-mapped encounters into the Range rooms.
4. **Daily** — one chart, one leaderboard, replays validated server-side.
5. **Ghosts** — race a replay drawn as a ghost crosshair and kill flashes. Live versus later.

### Mods (osu)

DT = play the chart at ×1.5 BPM (the BPM dial is the mod). HR = smaller weak points (`radius_exp`).
HD = no approach rings. NF = no fail. Mods multiply score and flag the leaderboard entry.

### Art direction

Borderlands ink and banded light; Cyberpunk 2077 palette and material language; enemies and weak
points are the icons and ship first. ASSETS.register.md is the art schedule; every row is VOID until
produced, imported, placeholder deleted, screenshot attached.

### World

Night. One city district per act: street level, interiors, metro, rooftops, a flooded underpass. Three
levels in act one. Rooms are authored with anchors (cover, perches, lanes, doors); patterns address
positions relative to the room's stand point so the same pattern plays in any room.

## 2. Architecture

- `core/` Rust (engine-independent, 36 tests): conductor, beat-space patterns and rooms, compiler,
  judge (ms windows, soft gating, frame-independent), enemy fire and HP, scoring, replays with chart
  fingerprint, song analyser, difficulty rating, bot simulator. GDExtension now; wasm for the editor.
- `godot/` 4.7: driver, stems, settings, scores, level builder (rooms, corridors, doors), player
  (movement, weapon), enemies (body + weak point), HUD, post-FX, autoplay bot, engine tests.
- `tools/`: one-command build/test/smoke, `dbmap`, `gdcheck`.

## 3. Phases

Each phase is a slice of every layer with an exit criterion that has to run green.

### P0 — Model and greybox lane (done)

### P1 — Shooter (now)

- Rooms, segments (traversal / encounter), enemy behaviours, enemy fire and HP in the model.
- Level builder: three greybox rooms with cover and height, corridors, doors on downbeats.
- Enemies as nodes: body (chip) and weak point (judged), approach ring, telegraph, death.
- Player: sprint, slide, jump, mantle; HP, damage flash, down.
- Weapon: tap/hold, recoil by verdict, beam for striders.
- First hardware run; latency budget (mouse to judge < 8 ms at 240 fps; calibration stable ±5 ms).
- Exit: play `street_128` start to finish on hardware; no judge surprises; movement feels like a shooter.

### P2 — Range and measurement

- Section restart on down (song rewound to the section's bar).
- Range: every pattern as a drill at any BPM; stats store per scenario and BPM; daily routine; ranks.
- Results screen (histogram, scatter, per-scenario table, replay).
- Settings and level select screens (greybox UI, real data). Mods.
- Exit: a 20-minute Range session end to end; a campaign level with one section restart.

### P3 — Content pipeline

- Any Song in-engine (analysis → grid → confirm → auto encounters).
- Chart editor on the web: wasm core + Svelte; rooms, anchors, pattern palette, per-enemy timing,
  playback against the song, export. *(Call: web editor, not in-engine.)*
- Three commissioned tracks with stems; act one levels charted by hand.
- Exit: one commissioned song charted in the editor and playable in its level.

### P4 — Campaign act one

- Three levels, set pieces, boss encounter, progression, cosmetics, second weapon decision.
- Exit: act one playable start to finish with saves.

### P5 — Art production

Register order: enemies and weak points, kill VFX, weapon, HUD, ink pass and ramps, rooms and city
kits, post, sky. Generative models for concept spread and texture bases; every shipped asset
hand-finished. Overlaps P4 from the moment enemies lock.

### P6 — Online: leaderboards with server re-scoring, daily, ghosts.

### P7 — Release: Steam, Next Fest demo (Range + one level + Any Song), Twitch integration, launch.

## 4. Verification, standing

Core `cargo test`; `dbsim --sweep`; `dbmap --synth`; engine `gdcheck`, `run_tests.gd`, autoplay smoke;
hardware calibration and latency; register rule for art.
