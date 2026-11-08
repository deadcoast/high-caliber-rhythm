#!/usr/bin/env bash
# One command: build the Rust core as a GDExtension and stage it for the Godot project.
#   tools/build.sh            release build
#   tools/build.sh --debug    debug build
#   tools/build.sh --run      build, then launch the warmup (needs `godot` on PATH or $GODOT)
#   tools/build.sh --smoke    build, then run the headless autoplay smoke test and check the report
#   tools/build.sh --test     build, then run the engine-side tests (godot/tests/run_tests.gd) headless
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
profile=release; flags=(build --features gdext --release); run=0; smoke=0; test=0; bpm=128
for a in "$@"; do case "$a" in
  --debug) profile=debug; flags=(build --features gdext);;
  --run) run=1;;
  --smoke) smoke=1;;
  --test) test=1;;
  --bpm=*) bpm="${a#--bpm=}";;
esac; done
echo "[highcal_rhythm] cargo ${flags[*]}"
(cd "$root/core" && cargo "${flags[@]}")
mkdir -p "$root/godot/bin"
case "$(uname -s)" in
  Darwin) cp "$root/core/target/$profile/libhighcal_rhythm_core.dylib" "$root/godot/bin/";;
  *) cp "$root/core/target/$profile/libhighcal_rhythm_core.so" "$root/godot/bin/";;
esac
echo "[highcal_rhythm] staged godot/bin ($profile)"
godot="${GODOT:-godot}"
if [ "$test" = 1 ] || [ "$smoke" = 1 ]; then
  "$godot" --headless --import --path "$root/godot" >/dev/null 2>&1 || true
fi
if [ "$test" = 1 ]; then
  "$godot" --headless --path "$root/godot" --script res://tests/run_tests.gd
fi
if [ "$smoke" = 1 ]; then
  echo "[highcal_rhythm] smoke: headless autoplay at $bpm BPM"
  out="$("$godot" --headless --path "$root/godot" -- --smoke --autoplay "--bpm=$bpm" 2>&1 || true)"
  line="$(printf '%s\n' "$out" | grep '^highcal_rhythm SMOKE ' | tail -1 || true)"
  if [ -z "$line" ]; then printf '%s\n' "$out"; echo "[highcal_rhythm] smoke: no report line (scene did not finish)"; exit 1; fi
  printf '%s\n' "${line#highcal_rhythm SMOKE }" | python3 -c '
import json,sys
r=json.load(sys.stdin)
print("[highcal_rhythm] smoke: grade %s  score %d  hits %d/%d  perfect %d  expired %d  whiffs %d  offset %+.1f ms  sd %.1f ms" % (r["grade"],r["score"],r["hits"],r["shots"],r["perfect"],r["expired"],r["whiffs"],r["timing_mean_ms"],r["timing_std_ms"]))
ok = r["expired"]==0 and r["whiffs"]==0 and r["perfect"]>=r["hits"]*0.95
print("[highcal_rhythm] smoke: PASS" if ok else "[highcal_rhythm] smoke: FAIL (the bot is exact; a miss is a scene bug)")
sys.exit(0 if ok else 1)'
fi
if [ "$run" = 1 ]; then "$godot" --path "$root/godot" -- "--bpm=$bpm"; fi
