# DOWNBEAT — Visual Asset Register

Every front-facing visual slot in the build. **VOID** = a placeholder primitive, colour or default font sits
in the slot today; the backend around the slot is real. Nothing marked VOID ships. When the backend is
coherent, each row gets a production pass in order: brief → generation (model-assisted) → cleanup → import
→ in-engine check → sign-off.

Art direction anchor: Borderlands ink-line cel shading (flat banded light, hand-inked contour, hatched
shadow texture) over a Cyberpunk 2077 palette and material language (wet asphalt, neon signage, hard
magenta/cyan key lights, chrome, decals). Targets are the game's icons: they must read in 150 ms at 12 m.

Columns: ID · Slot · Where it plugs in · State · Spec the production asset must meet · Route.

## SHD — Shading and post

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| SHD-01 | Cel light | `godot/shaders/cel.gdshader` (two-tone cut, cel specular, rim, optional `ramp_tex`) and `src/world/palette.gd` | VOID values, features final | 3–4 band ramp texture (1D, 256 px), hatch texture (tileable 1k, screen-space projected in shadow band), rim colour per light rig. Ramp authored per material family (concrete, chrome, skin, glass). | Hand-paint ramps; hatch generated then cleaned in a 2D tool; shader gains `ramp_tex`, `hatch_tex` uniforms. |
| SHD-02 | Ink line pass | `godot/shaders/outline.gdshader` (inverted hull) | VOID technique | Full-screen edge pass: Sobel on depth + normal with line weight by distance, thick silhouette, thin crease, per-material line colour (not pure black: deep violet). Hull pass retired. | Write as a CompositorEffect (Forward+); keep hull for far objects only. |
| SHD-03 | Beat-reactive post | `godot/shaders/postfx.gdshader`, `src/ui/postfx.gd` (beat-breathing chromatic split, scanlines, row glitch and red push on damage, Perfect flash, break desaturate; `video/postfx`, `aberration`, `scanlines`) | VOID look, features final | Chromatic split + vignette breathe keyed to `beat_phase`; hit flash on Perfect; desaturate on combo break. Strength capped so aim is never disturbed. | Replace the shader body; keep the uniforms. |

## ENV — Rooms and city

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| ENV-01 | Street room | `level.gd` from `rooms[]` (30×9×40, cover both sides, van) | VOID | Night street kit: wet asphalt tiles, kerbs, vending shells, a van hero prop, signage; cover silhouettes match the greybox blocks within 0.3 m. glTF, trim sheets, 2k. | Blockout is final; hero props modelled, kit textured (model-assisted bases, hand-cleaned). |
| ENV-02 | Atrium room | `level.gd` (26×16×30, two perches, pillar) | VOID | Corporate atrium: glass, chrome, holographic signage, perches as mezzanines. | As above. |
| ENV-03 | Platform room | `level.gd` (36×7×34, track trench, pylons) | VOID | Metro platform: tracks, third-rail glow, pylons, departure boards. | As above. |
| ENV-04 | Corridors | `level.gd` `_corridor` (6 wide, 5 high, light strips) | VOID | Alley / service corridor kit with the neon strips as real fixtures. Length is set by the song; the kit tiles. | Modular kit, 3 m module. |
| ENV-08 | Underpass room | `level.gd` from `rooms[]` (20×5×44, pillar rows, a wreck) + the water sheet (cue `flood`) | VOID | Flooded underpass: wet concrete, pillar rows, a car half under, caustics on the water; the flood is the set piece. | Kit + water shader. |
| ENV-09 | Lobby room | `level.gd` (28×12×30, desk, two mezzanines, columns) | VOID | Corporate lobby: marble, glass balustrades, holo signage over the desk. | Kit. |
| ENV-10 | Rooftop room | `level.gd` (34×8×36, HVAC, vents, deck, parapet) | VOID | Night rooftop over the district: HVAC, cabling, the parapet edge with the city past it. | Kit + skyline panorama (ENV-07). |
| ENV-11 | Helipad room | `level.gd` (40×12×40, pad lights, masts, crane base) | VOID | The helipad: pad markings, floodlights, the crane base the boss hangs from. | Set piece. |
| ENV-12 | Set pieces | `level.gd` `_on_cue` (train, lights, flood), `boss.gd` (reveal), `player.gd` (shake) | VOID look, timing final | The train as a modelled metro car with lit windows; light schemes as real fixture states; the flood with surface and splash. | VFX + models. |
| ENV-05 | Doors | `level.gd` door panels (4×3.2, slide up on the downbeat) | VOID look, timing final | Blast-door design with a beat-synced open; light bar shows bars until it opens. | Model + animation. |
| ENV-06 | Start pad | `level.gd` `_pad` | VOID | Rooftop/stairwell entry where the run begins. | Set piece. |
| ENV-07 | Sky / city | `level.gd` `_environment()` flat colour | VOID | Painted skyline panorama (8k×2k) with animated neon layers. | Model-generated panels, stitched, masked. |

## TGT — Enemies (the icons of the game)

Each enemy is a body (chips, never judged) and a weak point (the hit object) with an approach ring.
Bodies are greybox primitives in `enemy.gd`; exposure, telegraph and fire timing are final.

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| TGT-01 | DRONE (FLICK) | `enemy.gd` box body, magenta core; hook `assets/enemies/drone.glb` | VOID, brief written (`art/BRIEFS.enemies.txt` [2]) | Quad-rotor security drone; core on top; reads at 12 m; ≤3k tris. | Concept sheet → model → rig (hover) → cel material → drop at the hook path → screenshot. |
| TGT-02 | GUNNER (SWITCH) | `enemy.gd` capsule body, amber core; hook `assets/enemies/gunner.glb` | VOID, brief written ([3]) | Corporate gunner behind cover; peek animation on 8ths; core at the visor. | Model + peek anim. |
| TGT-03 | STRIDER (TRACK) | `enemy.gd` long capsule, cyan core, moves on its path; hook `assets/enemies/strider.glb` | VOID, brief written ([4]) | Quadruped runner; core on the spine; trail ribbon while in hold. | Model + run cycle + ribbon shader. |
| TGT-04 | SWARM (MICRO) | `enemy.gd` small sphere, lime core; hook `assets/enemies/swarm.glb` | VOID, brief written ([5]) | Palm-sized bots in formation; core is the eye; halo so 0.3 m reads at 12 m. | Model + halo billboard. |
| TGT-05 | HEAVY (RUSH) | `enemy.gd` large box per plate, yellow plates; hook `assets/enemies/heavy.glb` | VOID, brief written ([6]) | Walker with eight armour plates that light in sequence; charge telegraph on the last. Engine step: the spawner groups a rush phrase into one walker body. | Segment model + plate shader; then the grouped body. |
| TGT-06 | Approach ring | `enemy.gd` TorusMesh | VOID | The timing cue; collapses onto the core at the beat; last 100 ms hard lock frame. | Ring mesh + band shader. |
| TGT-07 | Kill VFX | `enemy.gd` `die("hit")` | VOID | Per-verdict: Perfect shatter + ring shock; Great shatter; Good puff. ≤120 ms. | Particle + shatter prefabs. |
| TGT-08 | Enemy fire | `enemy.gd` `_muzzle()` line | VOID | Tracer + muzzle flash toward the player; damage flash on HUD and post. | VFX + SFX. |
| TGT-10 | THE CRANE (boss) | `src/world/boss.gd` (hull, cabin, six plates, core, arms, lamps, cable; reveal rise, plate drops per phase, attack flash, kill collapse); parts tethered by `enemy.gd`; hook `assets/boss/crane.glb` with `plate_0..5`, `core` nodes adopted | VOID, brief written ([8]) | The act one boss: a crane-hung gunship; gun pods (phase 1), searchlights (phase 2), the core (phase 3); plates shed per phase; the kill as a fall into the city. | Concept → model → rig → per-phase states. |
| TGT-09 | Marked core / Downbeat volley | `enemy.gd` `set_marked` (yellow core), `hud.gd` charge bar, `Kit` VOLLEY stab | VOID | Mark brand on the core, bar-line pulse when ready, volley kill: every marked core dies in one synchronised shatter on the downbeat. | VFX + SFX + type. |

## WPN — Weapon

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| WPN-01 | Viewmodel | `src/player/weapon.gd` (idle sway on beat phase, recoil sized by verdict, hold shake, on-beat flash) | VOID mesh, motion hooks final | One hitscan pistol/SMG hybrid, cel-shaded, animated: idle, fire, hold loop. FOV-independent viewmodel camera. | Concept → model → rig; animations replace the procedural curves. |
| WPN-02 | Muzzle / tracer | none yet | VOID | Ink-style muzzle flash card set (4 frames), tracer only on Perfect. | Hand-drawn frames. |
| WPN-03 | Impact decal | none yet | VOID | On-beat vs off-beat decals on arena surfaces. | Decal textures. |
| WPN-04 | Dash and footsteps | `player.gd` `_dash`, `_on_beat` (STEP/DASH kit sounds, FOV punch) | VOID sound/VFX, feel final | Dash streak VFX (longer on-beat), footstep SFX per surface on the beat. | VFX + SFX. |

## HUD / UI

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| HUD-01 | Crosshair + beat ring | `hud.gd` `_draw_hud` arcs | VOID | Crosshair pack (FPS-standard shapes, user-colourable) + beat ring design; ring must not occlude the 0.3 m MICRO target at 12 m. | Vector art → SVG/PNG atlas. |
| HUD-02 | Judgement type | `hud.gd` fallback font | VOID | Display face for PERFECT/GREAT/GOOD/EARLY/LATE/MISS/WHIFF + animation (rise, kick, fade). | Typeface selection/licensing + motion spec. |
| HUD-03 | Timing meter | `hud.gd` rects | VOID | The aim-trainer stat: ±110 ms bar with history ghosts (last 8 shots), mean marker. | Vector UI. |
| HUD-04 | Score / combo / best | `hud.gd` strings (score, combo, BEST from `scores.gd`, stars) | VOID | Combo counter with tier colours at 10/25/50; score roll; best and stars. | Vector UI + type. |
| HUD-05 | Status bar | `hud.gd` bottom text | VOID | BPM, bar:beat, drift, offset — debug now, becomes a compact telemetry strip. | Vector UI. |
| HUD-06 | Results screen | `src/ui/results_screen.gd` (grade, score/pp/FC, judgement histogram, timing histogram and scatter, per-enemy table, leaderboard with integrity flags; R / Enter watch replay / Tab / Esc) | VOID look, data and flow final | Designed results page over the same data; grade stamp animation, type, histogram styling. | UI design → scenes. |
| UI-01 | Menus | `src/ui/menu.gd` (level select with builtins at any BPM, shipped and Any Song levels, stars and bests under mods; settings; pause), `levels.gd` index, `settings.gd` values, `range_menu.gd` (drill list, band rank, routine; Tab) | VOID screens, data and flow final | Main, level select (song art, difficulty ladder per song), Range (drill grid, BPM dial, rank, routine), settings (grouped, with a sensitivity converter), pause. | UI design → scenes. |
| UI-02 | Calibration screen | `db.gd` `start_calibration()`; result persisted by `settings.gd` | VOID look, logic final | Visual tap-to-beat screen with live offset readout and confidence. | UI design. |
| UI-03 | Any Song screens | `src/ui/any_song.gd` (native file picker, LISTENING label), `src/ui/song_confirm.gd` (grid onset bars, energy per bar, nudge keys, click preview) | VOID look, flow final | Picker with recent songs and art, confirm screen with a waveform, draggable beat-0 marker and tap-tempo; the click preview stays. | UI design → scenes. |
| UI-05 | Campaign | `menu.gd` act block (locks, bests, CLEAR), `campaign.gd` | VOID look, flow final | The act as a map of the district with the three levels lit as they clear; level cards with art. | UI design. |
| HUD-07 | Boss bar | `hud.gd` (remaining parts, phase kills vs clear line, bars left, phase name) | VOID look, data final | Boss bar with the phase ticks and the clear line as a designed element; phase lost/clear stamps. | Vector UI + type. |
| UI-06 | Online | `results_screen.gd` world rank and board, `menu.gd` DAILY row and online settings, `hud.gd` ghost crosshair and score delta, `enemy.gd` `ghost_flash` | VOID look, flow final | World board as a designed column with player cards; the daily as a banner with a countdown to the next; the ghost crosshair and ring in the HUD's own language. | UI design + VFX. |
| UI-04 | Replay watch | `src/core/replay_player.gd` (camera follows the recorded view track), `hud.gd` REPLAY strip | VOID look, logic final | Ghost crosshair and kill flashes drawn over the recording; scrub bar; becomes the Ghosts mode view. | UI design. |

## AUD — Audio

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| AUD-01 | Count-in / click | `click_track.gd` procedural (3 layers: pulse, hats, accent) | VOID | Produced count-in per BPM family; replaced per song by the song's own intro. | Produced. |
| AUD-02 | Kit samples | `src/core/kit.gd` synthesised kick/snare/hat/cymbal/tom/dead; `DB.play_kit` gain by verdict; per-level `kit` paths override | VOID sound, rule final | One kit per song key: FLICK kick, SWITCH snare, MICRO hat, TRACK cymbal, RUSH tom, DEAD rim. The player's drum part is these samples; the song's drum stem ships muted. | Produced per song. |
| AUD-03 | Music stems | `db.gd` `AudioStreamSynchronized`, tiers at combo 0/10/25/50, duck on break; level JSON `stems[]`, `song_offset_s` from `dbmap`; act one's three tracks rendered by `core/src/song.rs` (`dbsong`) into `godot/audio/` with their hand charts `levels/{night_market_132,undertow_150,skyline_168}.json` | VOID content (procedural placeholder tracks), system final | Per-song stems (drums, bass, lead, pad), sample-locked, beat-aligned at authoring BPM with the measured `song_offset_s`. Each plan is the brief for its commissioned replacement. | Licensing/production; `dbmap song.wav` for the grid. |

## Briefs

`art/BRIEFS.enemies.txt`: the enemies, their weak points, the shared weak-point language and the boss, with
the read numbers, offsets, clip names, budgets, import spec, hook paths and prompt stems. Reference sheets
land in `art/ref/<enemy>/`.

## Tracking rule

A row leaves VOID only when: the production asset is imported, the primitive it replaced is deleted from the
scene code (no fallback kept), and a screenshot at 1080p from the lane origin is attached to the row.
