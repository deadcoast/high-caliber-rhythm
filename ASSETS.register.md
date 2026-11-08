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
| SHD-01 | Toon ramp + hatch | `godot/shaders/cel.gdshader` (`bands`, `shadow_floor`, rim) | VOID values | 3–4 band ramp texture (1D, 256 px), hatch texture (tileable 1k, screen-space projected in shadow band), rim colour per light rig. Ramp authored per material family (concrete, chrome, skin, glass). | Hand-paint ramps; hatch generated then cleaned in a 2D tool; shader gains `ramp_tex`, `hatch_tex` uniforms. |
| SHD-02 | Ink line pass | `godot/shaders/outline.gdshader` (inverted hull) | VOID technique | Full-screen edge pass: Sobel on depth + normal with line weight by distance, thick silhouette, thin crease, per-material line colour (not pure black: deep violet). Hull pass retired. | Write as a CompositorEffect (Forward+); keep hull for far objects only. |
| SHD-03 | Beat-reactive post | `godot/shaders/postfx.gdshader`, `src/ui/postfx.gd` (hooks: beat phase, Perfect flash, break desaturate; strength = `video/postfx` setting, 0 = off) | VOID look, hooks final | Chromatic split + vignette breathe keyed to `beat_phase`; hit flash on Perfect; desaturate on combo break. Strength capped so aim is never disturbed. | Replace the shader body; keep the uniforms. |
| SHD-04 | Sky / backdrop | `arena.gd` `_environment()` flat colour | VOID | Painted city skyline panorama (8k×2k equirect) with animated neon layers (separate emissive masks). | Model-generated skyline panels → stitched, cleaned, masked. |

## ENV — Arena

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| ENV-01 | Lane floor | `arena.gd` floor box | VOID | Modular 4 m tiles, wet-asphalt + painted lane markings, trim sheet. glTF, ≤8k tris per tile, 2k atlas. | Blockout → hero tile modelled → trim sheet painted (model-assisted base, hand-cleaned). |
| ENV-02 | Backdrop wall | `arena.gd` backdrop box at z −20 | VOID | The read surface behind targets: low-contrast, dark, no high-frequency detail in the 50° cone the targets occupy; signage only outside that cone. | Modelled set piece; signage decals. |
| ENV-03 | Side walls / blocks | `arena.gd` side boxes, pillars | VOID | Kit of 6–8 cyberpunk street props (vending shell, cable trunk, AC stack, barrier) with ink-line-friendly silhouettes. | Kitbash → retopo → cel materials. |
| ENV-04 | Neon strips | `arena.gd` `_strip()` emissive boxes | VOID | Signage meshes with emissive masks; colour per lane state (idle, run, combo tiers). | Modelled + emissive mask textures. |
| ENV-05 | Lane origin mark | `arena.gd` cyan pad | VOID | Floor decal: player start, faces lane. | Decal texture. |

## TGT — Targets (the icons of the game)

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| TGT-01 | FLICK body | `target.gd` SphereMesh | VOID | Hard-edged drone/orb silhouette, magenta family, reads as a sphere for hitbox honesty (hitbox stays the sphere `radius`). ≤2k tris. | Concept sheet (model-assisted) → modelled → cel material. |
| TGT-02 | SWITCH body | `target.gd` SphereMesh, amber | VOID | Same family, amber, distinct silhouette flag so two live targets are told apart at a glance. | As above. |
| TGT-03 | TRACK body | `target.gd` SphereMesh, cyan | VOID | Elongated along motion, cyan, trail ribbon while in hold. | Model + ribbon shader. |
| TGT-04 | MICRO body | `target.gd` SphereMesh, lime, r 0.3 | VOID | Small, high-contrast core + thin halo so 0.3 m reads at 12 m. | Model + halo billboard. |
| TGT-08 | RUSH body | `target.gd` SphereMesh, yellow | VOID | Stacked short-lived fill drum; reads as a sequence (numbered or trailing) so eight in two beats are legible. | Model + sequence shader. |
| TGT-05 | Approach ring | `target.gd` TorusMesh | VOID | The timing cue. Ring art that collapses onto the body at `hit_time`; last 100 ms gets a hard “lock” frame. Must stay legible over any backdrop. | Shader-driven ring mesh with band texture. |
| TGT-06 | Hit pop | `target.gd` `die("hit")` scale burst | VOID | Per-timing VFX: Perfect = full shatter + ring shock; Great = shatter; Good = puff. ≤120 ms. | Particle + mesh-shatter prefabs, cel-shaded. |
| TGT-07 | Expire / off-beat | `target.gd` `die("expired")` grey shrink | VOID | Desaturate + collapse; off-beat hit gets a dull thud flash (no shatter). | Particle + material swap. |

## WPN — Weapon

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| WPN-01 | Viewmodel | `src/player/weapon.gd` (idle sway on beat phase, recoil sized by verdict, hold shake, on-beat flash) | VOID mesh, motion hooks final | One hitscan pistol/SMG hybrid, cel-shaded, animated: idle, fire, hold loop. FOV-independent viewmodel camera. | Concept → model → rig; animations replace the procedural curves. |
| WPN-02 | Muzzle / tracer | none yet | VOID | Ink-style muzzle flash card set (4 frames), tracer only on Perfect. | Hand-drawn frames. |
| WPN-03 | Impact decal | none yet | VOID | On-beat vs off-beat decals on arena surfaces. | Decal textures. |

## HUD / UI

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| HUD-01 | Crosshair + beat ring | `hud.gd` `_draw_hud` arcs | VOID | Crosshair pack (FPS-standard shapes, user-colourable) + beat ring design; ring must not occlude the 0.3 m MICRO target at 12 m. | Vector art → SVG/PNG atlas. |
| HUD-02 | Judgement type | `hud.gd` fallback font | VOID | Display face for PERFECT/GREAT/GOOD/EARLY/LATE/MISS/WHIFF + animation (rise, kick, fade). | Typeface selection/licensing + motion spec. |
| HUD-03 | Timing meter | `hud.gd` rects | VOID | The aim-trainer stat: ±110 ms bar with history ghosts (last 8 shots), mean marker. | Vector UI. |
| HUD-04 | Score / combo / best | `hud.gd` strings (score, combo, BEST from `scores.gd`, stars) | VOID | Combo counter with tier colours at 10/25/50; score roll; best and stars. | Vector UI + type. |
| HUD-05 | Status bar | `hud.gd` bottom text | VOID | BPM, bar:beat, drift, offset — debug now, becomes a compact telemetry strip. | Vector UI. |
| HUD-06 | Results screen | `hud.gd` `_on_finished` label + `_draw_scatter` (reads `report.history`) | VOID look, data final | Full results page: grade stamp, judgement histogram, timing scatter (ms vs beat), per-kind table, replay button (replays are saved to `user://replays`). | UI design → scenes. |
| UI-01 | Menus | `levels.gd` index (rating, bars, best per level), `settings.gd` values; `--list-levels` | VOID screens, data final | Main, level select (BPM, stars, best, song art), settings (sens cm/360 + DPI, FOV, calibration, post-FX strength), pause. | UI design → scenes. |
| UI-02 | Calibration screen | `db.gd` `start_calibration()`; result persisted by `settings.gd` | VOID look, logic final | Visual tap-to-beat screen with live offset readout and confidence. | UI design. |

## AUD — Audio

| ID | Slot | Where | State | Spec | Route |
|---|---|---|---|---|---|
| AUD-01 | Count-in / click | `click_track.gd` procedural (3 layers: pulse, hats, accent) | VOID | Produced count-in per BPM family; replaced per song by the song's own intro. | Produced. |
| AUD-02 | Kit samples | `src/core/kit.gd` synthesised kick/snare/hat/cymbal/tom/dead; `DB.play_kit` gain by verdict; per-level `kit` paths override | VOID sound, rule final | One kit per song key: FLICK kick, SWITCH snare, MICRO hat, TRACK cymbal, RUSH tom, DEAD rim. The player's drum part is these samples; the song's drum stem ships muted. | Produced per song. |
| AUD-03 | Music stems | `db.gd` `AudioStreamSynchronized`, tiers at combo 0/10/25/50, duck on break; level JSON `stems[]`, `song_offset_s` from `dbmap` | VOID content, system final | Per-song stems (drums, bass, lead, pad), sample-locked, beat-aligned at authoring BPM with the measured `song_offset_s`. | Licensing/production; `dbmap song.wav` for the grid. |

## Tracking rule

A row leaves VOID only when: the production asset is imported, the primitive it replaced is deleted from the
scene code (no fallback kept), and a screenshot at 1080p from the lane origin is attached to the row.
