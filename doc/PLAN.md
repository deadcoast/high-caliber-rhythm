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
4. **Daily** — one chart, one leaderboard, replays validated server-side. Built.
5. **Ghosts** — race a replay drawn as a ghost crosshair and kill flashes. Built (from a board's #1).

### The Downbeat (signature mechanic)

Sixteen on-beat kills charge it. Hold the right button to mark exposed weak points, release exactly on
the bar line: everything marked dies at once with that one timing, full combo credit. Release off the
bar and the charge, the marks and the combo are gone. It is the McCree moment made a skill shot: the
volley on the one. *(Call: mine, from the brief's origin. Charge count, the bar as the only release
point and the all-or-nothing penalty are the tuning surface.)*

### Movement on the beat

Movement is never gated, and still reads the music: the head bob lands on the beat, footsteps sound on
the beat, the dash (Q) recharges on each bar and goes 40% further without spending the charge when it
starts inside ±90 ms of a beat. Camera punch sizes itself by the verdict; none of it touches the clock.

### Mods (osu)

DT ×1.5 BPM (1.12), HT ×0.75 (0.30), HR weak points at 75% (1.06), HD no approach rings (1.06), NF no
fail (0.50). Multipliers stack. The mod set is part of the chart fingerprint and the leaderboard key.

### Ranking (osu)

Every play gets a performance value: stars² × 50 × composite³ × (0.6 + 0.4 × combo ratio) × mods. The
profile rating is the osu-weighted sum of the best play per chart (0.95ⁱ). Leaderboards are per chart
and mod set, top 10 with replays. Every replay is re-scored and inspected: timing spread below 3 ms,
precision above 0.995, sixteenth-quantised cadences and out-of-order inputs hold a score for review.
Grades S–F, FC and PFC flags.

### Art direction

Edgerunners (Trigger) through the Cyberpunk 2077 universe: hard two-tone cel light with a crisp cut,
small hard specular, selective ink, acid yellow / cyan / hot magenta on violet-black, chromatic split
that breathes with the beat, row glitch on damage. One palette file drives every material and HUD
colour. Enemies and weak points are the icons and ship first. ASSETS.register.md is the art schedule; every row is VOID until
produced, imported, placeholder deleted, screenshot attached.

### World

Night. One city district per act: street level, interiors, metro, rooftops, a flooded underpass. Three
levels in act one. Rooms are authored with anchors (cover, perches, lanes, doors); patterns address
positions relative to the room's stand point so the same pattern plays in any room.

## 2. Architecture

- `core/` Rust (engine-independent, 54 tests): conductor, beat-space patterns and rooms, compiler
  (encounters, boss phases, cues), judge (ms windows, soft gating, frame-independent, phase close),
  enemy fire and HP, section and phase restart, mods, the Downbeat volley, scoring with pp and FC
  flags, replays with chart fingerprint and live watching, integrity checks, song decoder and
  analyser, track renderer, difficulty rating, drills and ranks, bot simulator. GDExtension for the
  game; a C ABI on wasm32 for the editor.
- `godot/` 4.7: driver, stems, settings, scores, campaign, level builder (rooms, corridors, doors, set
  pieces), player (movement, weapon), enemies (body + weak point), the boss, HUD with the boss bar,
  results screen, menu, Range, Any Song, replay watching, post-FX, autoplay bot, engine tests.
- `web/`: the chart editor (Svelte 5 + Vite over the wasm core).
- `server/`: the online layer (axum + SQLite), one binary, Dockerfile.
- `tools/`: one-command build/test/smoke/web/server, `dbmap`, `gdcheck`.

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

- Section restart on down (song rewound to the section's bar). Done.
- Range: every pattern as a drill at any BPM; stats store per scenario and BPM; daily routine; ranks. Done.
- Results screen (histograms, scatter, per-enemy table, leaderboard, watch the replay). Done; replays are
  watched live through the model with the recorded camera.
- Settings and level select screens (greybox UI, real data). Done: menu with level select (builtins at
  any BPM, shipped and Any Song levels, stars and bests under the chosen mods), settings, pause.
- Exit: a 20-minute Range session end to end; a campaign level with one section restart.

### P3 — Content pipeline

- Any Song in-engine (analysis → grid → confirm → auto encounters). Done: any format the core decodes,
  worker-thread analysis, grid confirmed by ear against a rendered click, level into `user://songs`.
- Chart editor on the web: wasm core + Svelte; rooms, path, pattern palette and aim canvas, per-enemy
  timing, playback against the song, rating and bot run, export. Done (`web/`). *(Call: web editor, not
  in-engine.)*
- Three commissioned tracks with stems; act one levels charted by hand. First track in place as a
  produced placeholder: Night Market (132 BPM, 88 bars, four stems rendered by the core) with a hand
  chart in the editor's format, aligned to its sections and cleared by the bot. Commissioning replaces
  the stems, nothing else.
- Exit: one commissioned song charted in the editor and playable in its level.

### P4 — Campaign act one (now)

- Three levels charted on three produced tracks (Night Market, Undertow, Skyline), seven rooms. Done.
- Set pieces as chart cues: train, lights, flood, reveal, shake. Done.
- Boss encounter as a chart within the chart: phases on the song's sections, clear ratios, phase
  restart, the kill. Done (THE CRANE on the helipad).
- Progression and saves: act one unlocks level by level on a C, bests kept per level. Done.
- Cosmetics and the second weapon decision: open. *(Call: the second weapon waits for hardware feel on
  the first; cosmetics wait for the art pass so they are real.)*
- Exit: act one playable start to finish with saves — on hardware.

### P5 — Art production

Register order: enemies and weak points, kill VFX, weapon, HUD, ink pass and ramps, rooms and city
kits, post, sky. Generative models for concept spread and texture bases; every shipped asset
hand-finished. Overlaps P4 from the moment enemies lock.

### P6 — Online (done, pending a deployment)

- `server/`: replay re-scoring with the core, integrity, ranked set, boards per chart and mod set, the
  daily from the date, replay download, profile rating. Client: automatic posting, world rank and board
  on the results screen, the daily on the menu, ghost races from a board's top replay.
- Exit: a deployed server behind TLS and the daily played by two machines on the same board.

### P7 — Release: Steam, Next Fest demo (Range + one level + Any Song), Twitch integration, launch.

## 4. Verification, standing

Core `cargo test`; `dbsim --sweep`; `dbmap --synth`; engine `gdcheck`, `run_tests.gd`, autoplay smoke;
hardware calibration and latency; register rule for art.
