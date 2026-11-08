# DOWNBEAT chart editor (web)

The same Rust core as the game, built to wasm, behind a Svelte 5 UI. Charts are authored in beat space
against the song's measured grid and exported as the level JSON the engine loads.

```
npm install
npm run core     # cargo build --target wasm32-unknown-unknown (rustup target add wasm32-unknown-unknown) + stage
npm run check    # exercise every core op from Node: compile, rate, simulate, analyse, regrid, auto level
npm run dev      # live editing at http://localhost:5173
npm run build    # static site in dist/
```

`tools/build.ps1 -Web` / `tools/build.sh --web` runs all of it.

What it does: drop a song (any format the browser decodes) → tempo and beat 0 measured by the core's
analyser, nudge by ear with the click rendered on the grid → `use this grid` writes bpm and beat 0 into
the level → `auto chart from energy` lays encounters by the song's energy per bar, or build the path by hand:
rooms, traversals (bars), encounters (patterns × repeats). Inline patterns are edited on the aim canvas
(yaw across, pitch up, drag the weak points, click to add at a beat; striders get a path end) and in the
table. The timeline shows every target by enemy lane with its lead and hold, the segment spans, the song's
energy and onsets, the strain curve and the playhead; playback plays the song with the kit on every target.
The rating (stars and its parts) updates on every edit and the bot from `dbsim` can play the chart in the
browser. Export gives `godot/levels/<name>.json`; the level's `audio` must point at the song under
`godot/audio/` (`res://audio/<file>`).

Core protocol: `core/src/wasm.rs` (JSON over a C ABI, no bindgen), client in `src/lib/core.js`.
