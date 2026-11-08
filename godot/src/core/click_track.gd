class_name ClickTrack
extends RefCounted
## Procedural count-in / metronome stream at any BPM, built in memory so the
## conductor has a real audio clock to lock to before a song exists.
## SCAFFOLD AUDIO: replaced by the licensed/produced track stems. See ASSETS.register.md [AUD-01].

const SAMPLE_RATE := 44100


var _data := PackedByteArray()


## The whole click as one stream.
static func build(bpm: float, beats: int, beats_per_bar: int = 4) -> AudioStreamWAV:
	return ClickTrack.new()._build(bpm, beats, beats_per_bar, "all")


## Three sample-aligned layers for the stem system: [pulse, hats, accent].
## Each is the full length so an AudioStreamSynchronized keeps them locked.
static func build_layers(bpm: float, beats: int, beats_per_bar: int = 4) -> Array[AudioStreamWAV]:
	var out: Array[AudioStreamWAV] = []
	for layer in ["pulse", "hats", "accent"]:
		out.append(ClickTrack.new()._build(bpm, beats, beats_per_bar, layer))
	return out


func _build(bpm: float, beats: int, beats_per_bar: int, layer: String) -> AudioStreamWAV:
	var spb := 60.0 / bpm
	var total_frames := int(ceil(beats * spb * SAMPLE_RATE)) + SAMPLE_RATE
	_data.resize(total_frames * 2)
	_data.fill(0)
	var down := _tone(1320.0, 0.055, 0.9)
	var quarter := _tone(880.0, 0.040, 0.6)
	var eighth := _noise(0.012, 0.18)
	var accent := _tone(2640.0, 0.030, 0.35)
	for b in beats:
		var frame := int(round(b * spb * SAMPLE_RATE))
		var downbeat := b % beats_per_bar == 0
		if layer == "all" or layer == "pulse":
			_mix(frame, down if downbeat else quarter)
		if layer == "all" or layer == "hats":
			_mix(frame + int(round(spb * 0.5 * SAMPLE_RATE)), eighth)
		if layer == "accent" and downbeat:
			_mix(frame, accent)
	var stream := AudioStreamWAV.new()
	stream.format = AudioStreamWAV.FORMAT_16_BITS
	stream.mix_rate = SAMPLE_RATE
	stream.stereo = false
	stream.data = _data
	return stream


static func _tone(freq: float, seconds: float, gain: float) -> PackedFloat32Array:
	var n := int(seconds * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	for i in n:
		var t := float(i) / SAMPLE_RATE
		var env := 1.0 - float(i) / n
		env = env * env
		out[i] = sin(TAU * freq * t) * env * gain
	return out


static func _noise(seconds: float, gain: float) -> PackedFloat32Array:
	var n := int(seconds * SAMPLE_RATE)
	var out := PackedFloat32Array()
	out.resize(n)
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	for i in n:
		var env := 1.0 - float(i) / n
		out[i] = rng.randf_range(-1.0, 1.0) * env * gain
	return out


func _mix(frame: int, sample: PackedFloat32Array) -> void:
	var limit := _data.size() / 2
	for i in sample.size():
		var f := frame + i
		if f < 0 or f >= limit:
			continue
		var cur := _data.decode_s16(f * 2) / 32767.0
		var v := clampf(cur + sample[i], -1.0, 1.0)
		_data.encode_s16(f * 2, int(v * 32767.0))
