# One command: build the Rust core as a GDExtension and stage it for the Godot project.
#   .\tools\build.ps1            release build
#   .\tools\build.ps1 -Debug     debug build
#   .\tools\build.ps1 -Run       build, then launch the warmup in Godot (needs `godot` on PATH or $env:GODOT)
#   .\tools\build.ps1 -Smoke     build, then run the headless autoplay smoke test and check the report
#   .\tools\build.ps1 -Test      build, then run the engine-side tests (godot/tests/run_tests.gd) headless
param([switch]$Debug, [switch]$Run, [switch]$Smoke, [switch]$Test, [double]$Bpm = 128)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$profile = if ($Debug) { "debug" } else { "release" }
$flags = @("build", "--features", "gdext")
if (-not $Debug) { $flags += "--release" }
Write-Host "[downbeat] cargo $($flags -join ' ')"
Push-Location "$root\core"
try { cargo @flags } finally { Pop-Location }
New-Item -ItemType Directory -Force "$root\godot\bin" | Out-Null
Copy-Item "$root\core\target\$profile\downbeat_core.dll" "$root\godot\bin\downbeat_core.dll" -Force
Write-Host "[downbeat] staged godot\bin\downbeat_core.dll ($profile)"
$godot = if ($env:GODOT) { $env:GODOT } else { "godot" }
if ($Test -or $Smoke) {
    # First headless run of a fresh checkout: build the import cache and class list.
    & $godot --headless --import --path "$root\godot" 2>&1 | Out-Null
}
if ($Test) {
    & $godot --headless --path "$root\godot" --script res://tests/run_tests.gd
    if ($LASTEXITCODE -ne 0) { throw "[downbeat] engine tests FAILED" }
}
if ($Smoke) {
    Write-Host "[downbeat] smoke: headless autoplay at $Bpm BPM"
    $out = & $godot --headless --path "$root\godot" -- --smoke --autoplay "--bpm=$Bpm" 2>&1
    $line = $out | Where-Object { $_ -match "^DOWNBEAT SMOKE " } | Select-Object -Last 1
    if (-not $line) { $out | Write-Host; throw "[downbeat] smoke: no report line (scene did not finish)" }
    $r = ($line -replace "^DOWNBEAT SMOKE ", "") | ConvertFrom-Json
    Write-Host ("[downbeat] smoke: grade {0}  score {1}  hits {2}/{3}  perfect {4}  expired {5}  whiffs {6}  offset {7:N1} ms  sd {8:N1} ms" -f $r.grade, $r.score, $r.hits, $r.shots, $r.perfect, $r.expired, $r.whiffs, $r.timing_mean_ms, $r.timing_std_ms)
    if ($r.expired -gt 0 -or $r.whiffs -gt 0 -or $r.perfect -lt ($r.hits * 0.95)) { throw "[downbeat] smoke: FAIL (the bot is exact; a miss is a scene bug)" }
    Write-Host "[downbeat] smoke: PASS"
}
if ($Run) {
    & $godot --path "$root\godot" -- "--bpm=$Bpm"
}
