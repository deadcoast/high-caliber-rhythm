class_name Kit
extends RefCounted
## The drum kit the player plays with the gun. One sample per target kind plus
## DEAD for off-beat shots. Synthesised in memory here so the drummer rule is
## audible before produced sound exists (ASSETS.register.md [AUD-02]); a level
## can supply its own samples through `kit` in its JSON.

const SAMPLE_RATE := 44100
const KINDS := ["FLICK", "SWITCH", "MICRO", "TRACK", "RUSH", "DEAD", "STEP", "DASH", "VOLLEY"]


## Build the full kit. Keys are kind labels; values AudioStreamWAV.
static func build() -> Dictionary:
	var k := Kit.new()
	return {
		"FLICK": k._wav(k._kick()),
		"SWITCH": k._wav(k._snare()),
		"MICRO": k._wav(k._hat()),
		"TRACK": k._wav(k._cymbal()),
		"RUSH": k._wav(k._tom()),
		"DEAD": k._wav(k._dead()),
		"STEP": k._wav(k._step()),
		"DASH": k._wav(k._dash()),
		"VOLLEY": k._wav(k._volley()),
	}


## Load a level's own kit; any kind missing falls back to the synthesised one.
static func load_paths(paths: Dictionary) -> Dictionary:
	var kit := build()
	for kind in paths:
		var s = load(str(paths[kind]))
		if s == null:
			push_error("kit sample not found: " + str(paths[kind]))
		else:
			kit[kind] = s
	return kit


func _kick() -> PackedFloat32Array:
	# Pitch sweep 150 -> 48 Hz with a hard transient; the downbeat voice.
	var n := int(0.16 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var phase := 0.0
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var f := 48.0 + 102.0 * exp(-t * 28.0)
		phase += TAU * f / SAMPLE_RATE
		var env := exp(-t * 14.0)
		var click := exp(-t * 400.0) * 0.6
		out[i] = (sin(phase) * env + click) * 0.95
	return out


func _snare() -> PackedFloat32Array:
	var n := int(0.14 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 11
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var body := sin(TAU * 185.0 * t) * exp(-t * 30.0) * 0.5
		var snap := rng.randf_range(-1.0, 1.0) * exp(-t * 22.0) * 0.55
		out[i] = body + snap
	return out


func _hat() -> PackedFloat32Array:
	var n := int(0.045 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 13
	var prev := 0.0
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var white := rng.randf_range(-1.0, 1.0)
		# one-pole high-pass keeps it bright
		var hp := white - prev
		prev = white
		out[i] = hp * exp(-t * 90.0) * 0.5
	return out


func _cymbal() -> PackedFloat32Array:
	var n := int(0.9 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 17
	var prev := 0.0
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var white := rng.randf_range(-1.0, 1.0)
		var hp := white - prev * 0.6
		prev = white
		var shimmer := sin(TAU * 3100.0 * t) * 0.15 + sin(TAU * 4700.0 * t) * 0.1
		out[i] = (hp * 0.35 + shimmer) * exp(-t * 4.0)
	return out


func _tom() -> PackedFloat32Array:
	var n := int(0.13 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var phase := 0.0
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var f := 118.0 + 60.0 * exp(-t * 40.0)
		phase += TAU * f / SAMPLE_RATE
		out[i] = sin(phase) * exp(-t * 18.0) * 0.8
	return out


func _dead() -> PackedFloat32Array:
	# A muted thud: the drummer hit the rim.
	var n := int(0.07 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 19
	for i in n:
		var t := float(i) / SAMPLE_RATE
		out[i] = (sin(TAU * 70.0 * t) * 0.4 + rng.randf_range(-1.0, 1.0) * 0.12) * exp(-t * 60.0)
	return out


func _step() -> PackedFloat32Array:
	# Boot on concrete: short filtered noise with a low thump.
	var n := int(0.05 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 23
	for i in n:
		var t := float(i) / SAMPLE_RATE
		out[i] = (rng.randf_range(-1.0, 1.0) * 0.25 + sin(TAU * 90.0 * t) * 0.3) * exp(-t * 70.0)
	return out


func _dash() -> PackedFloat32Array:
	# Rising whoosh over 180 ms.
	var n := int(0.18 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 29
	var prev := 0.0
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var white := rng.randf_range(-1.0, 1.0)
		var lp := prev + (white - prev) * clampf(0.05 + t * 2.0, 0.0, 1.0)
		prev = lp
		var env := sin(PI * t / 0.18)
		out[i] = lp * env * 0.6
	return out


func _volley() -> PackedFloat32Array:
	# The Downbeat: a detuned stab with a long tail.
	var n := int(0.6 * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var env := exp(-t * 5.0)
		out[i] = (sin(TAU * 220.0 * t) + sin(TAU * 223.0 * t) * 0.8 + sin(TAU * 440.0 * t) * 0.4 + sin(TAU * 55.0 * t) * 0.9) * 0.3 * env
	return out


func _wav(samples: PackedFloat32Array) -> AudioStreamWAV:
	var data := PackedByteArray()
	data.resize(samples.size() * 2)
	for i in samples.size():
		data.encode_s16(i * 2, int(clampf(samples[i], -1.0, 1.0) * 32767.0))
	var stream := AudioStreamWAV.new()
	stream.format = AudioStreamWAV.FORMAT_16_BITS
	stream.mix_rate = SAMPLE_RATE
	stream.stereo = false
	stream.data = data
	return stream
