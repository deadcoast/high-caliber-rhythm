#!/usr/bin/env bash
# One command: build the Rust core as a GDExtension and stage it for the Godot project.
#   tools/build.sh            release build
#   tools/build.sh --debug    debug build
#   tools/build.sh --run      build, then launch the warmup (needs `godot` on PATH or $GODOT)
#   tools/build.sh --smoke    build, then run the headless autoplay smoke test and check the report
#   tools/build.sh --test     build, then run the engine-side tests (godot/tests/run_tests.gd) headless
#   tools/build.sh --web      build the wasm core and the web chart editor (web/dist), run its checks
#                             (needs node + npm and `rustup target add wasm32-unknown-unknown`)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
#   tools/build.sh --server   build and test the online server (server/), print the run line
profile=release; flags=(build --features gdext --release); run=0; smoke=0; test=0; web=0; server=0; bpm=128
for a in "$@"; do case "$a" in
  --debug) profile=debug; flags=(build --features gdext);;
  --run) run=1;;
  --smoke) smoke=1;;
  --test) test=1;;
  --web) web=1;;
  --server) server=1;;
  --bpm=*) bpm="${a#--bpm=}";;
esac; done
echo "[downbeat] cargo ${flags[*]}"
(cd "$root/core" && cargo "${flags[@]}")
mkdir -p "$root/godot/bin"
case "$(uname -s)" in
  Darwin) cp "$root/core/target/$profile/libdownbeat_core.dylib" "$root/godot/bin/";;
  *) cp "$root/core/target/$profile/libdownbeat_core.so" "$root/godot/bin/";;
esac
echo "[downbeat] staged godot/bin ($profile)"
if [ ! -f "$root/godot/audio/night_market_base.wav" ] || [ ! -f "$root/godot/audio/undertow_base.wav" ] || [ ! -f "$root/godot/audio/skyline_base.wav" ]; then
  # The produced placeholder tracks: four stems each, rendered by the core (ASSETS.register.md [AUD-03]).
  echo "[downbeat] rendering act one's stems into godot/audio"
  (cd "$root/core" && cargo run --release --bin dbsong -- --out "$root/godot/audio")
fi
godot="${GODOT:-godot}"
if [ "$test" = 1 ] || [ "$smoke" = 1 ]; then
  "$godot" --headless --import --path "$root/godot" >/dev/null 2>&1 || true
fi
if [ "$test" = 1 ]; then
  "$godot" --headless --path "$root/godot" --script res://tests/run_tests.gd
fi
if [ "$smoke" = 1 ]; then
  echo "[downbeat] smoke: headless autoplay at $bpm BPM"
  out="$("$godot" --headless --path "$root/godot" -- --smoke --autoplay "--bpm=$bpm" 2>&1 || true)"
  line="$(printf '%s\n' "$out" | grep '^DOWNBEAT SMOKE ' | tail -1 || true)"
  if [ -z "$line" ]; then printf '%s\n' "$out"; echo "[downbeat] smoke: no report line (scene did not finish)"; exit 1; fi
  printf '%s\n' "${line#DOWNBEAT SMOKE }" | python3 -c '
import json,sys
r=json.load(sys.stdin)
print("[downbeat] smoke: grade %s  score %d  hits %d/%d  perfect %d  expired %d  whiffs %d  offset %+.1f ms  sd %.1f ms" % (r["grade"],r["score"],r["hits"],r["shots"],r["perfect"],r["expired"],r["whiffs"],r["timing_mean_ms"],r["timing_std_ms"]))
ok = r["expired"]==0 and r["whiffs"]==0 and r["perfect"]>=r["hits"]*0.95
print("[downbeat] smoke: PASS" if ok else "[downbeat] smoke: FAIL (the bot is exact; a miss is a scene bug)")
sys.exit(0 if ok else 1)'
fi
if [ "$web" = 1 ]; then
  (cd "$root/web" && { [ -d node_modules ] || npm install; } && npm run core && npm run check && npm run build)
  echo "[downbeat] editor built: web/dist (serve it, or 'npm run dev' in web/ for live editing)"
fi
if [ "$server" = 1 ]; then
  (cd "$root/server" && cargo test --release && cargo build --release)
  echo "[downbeat] server: server/target/release/dbserver --bind 0.0.0.0:8787 --db downbeat.db --levels ../godot/levels"
fi
if [ "$run" = 1 ]; then "$godot" --path "$root/godot" -- "--bpm=$bpm"; fi
