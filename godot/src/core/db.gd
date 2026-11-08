extends Node
## DB: the game driver (autoload).
## Owns the Rust game model (DbGame from downbeat_core), the audio clock, the
## stem mixer and the event bus. Everything in the scene listens to this node;
## nothing else touches the model.

signal run_started(bpm: float)
signal run_finished(report: Dictionary)
signal tick(index: int, beat: int, sub: int)
signal beat(index: int, in_bar: int)
signal bar(index: int)
signal target_spawned(id: int, info: Dictionary)
signal target_hit(id: int, verdict: Dictionary)
signal whiff(verdict: Dictionary)
signal target_expired(id: int)
signal track_done(id: int, verdict: Dictionary)
signal calibrated(input_offset_ms: float)
signal tier_changed(tier: int)
signal combo_broken()

const PREROLL_S := 2.0
const CLICK_BEATS := 128
## Combo counts at which each band stem tier opens. Tier 0 is the base layer alone.
const TIER_COMBOS := [0, 10, 25, 50]
const DUCK_DB := -9.0
const DUCK_S := 0.35
const REPLAY_DIR := "user://replays"
## Kit gain by verdict: the drummer rule. Off-beat plays DEAD.
const KIT_DB := {"PERFECT": 0.0, "GREAT": -3.0, "GOOD": -7.0}
const DEAD_DB := -6.0
const MUTED_DB := -80.0

var game = null
var player: AudioStreamPlayer
var kit_player: AudioStreamPlayer
var kit: Dictionary = {}
var running := false
var level_path := ""
var builtin := "warmup"
var bpm := 128.0
var tier := 0
var calibrating := false
var last_replay_path := ""

var _frame_usec: int = 0
var _stream_started := false
var _song_offset_s := 0.0
var _input_offset_ms := 0.0
var _cal_samples: Array[float] = []
var _stem_count := 0
var _band: Array[int] = []
var _drum_stem := -1
var _drum_live := true
var _first_spawn_time := INF
var _duck_tween: Tween


func _ready() -> void:
	_setup_input_map()
	player = AudioStreamPlayer.new()
	player.name = "Music"
	player.bus = "Master"
	add_child(player)
	kit_player = AudioStreamPlayer.new()
	kit_player.name = "Kit"
	kit_player.bus = "Master"
	kit_player.stream = AudioStreamPolyphonic.new()
	(kit_player.stream as AudioStreamPolyphonic).polyphony = 24
	add_child(kit_player)
	_input_offset_ms = float(Settings.get_value("input", "offset_ms"))
	if not ClassDB.class_exists("DbGame"):
		push_error("downbeat_core GDExtension not loaded. Run tools/build.ps1 (Windows) or tools/build.sh, then restart.")
		return
	game = ClassDB.instantiate("DbGame")


func _setup_input_map() -> void:
	var defs := {
		"fire": [_mouse(MOUSE_BUTTON_LEFT)],
		"move_forward": [_key(KEY_W)],
		"move_back": [_key(KEY_S)],
		"move_left": [_key(KEY_A)],
		"move_right": [_key(KEY_D)],
		"jump": [_key(KEY_SPACE)],
		"restart": [_key(KEY_R)],
		"release_mouse": [_key(KEY_ESCAPE)],
		"calibrate": [_key(KEY_C)],
	}
	for action in defs:
		if not InputMap.has_action(action):
			InputMap.add_action(action)
			for ev in defs[action]:
				InputMap.action_add_event(action, ev)


func _mouse(button: int) -> InputEventMouseButton:
	var e := InputEventMouseButton.new()
	e.button_index = button
	return e


func _key(code: int) -> InputEventKey:
	var e := InputEventKey.new()
	e.physical_keycode = code
	return e


## Load the builtin warmup chart at a BPM, backed by the procedural click layers.
func start_warmup(at_bpm: float) -> void:
	if game == null:
		return
	level_path = ""
	builtin = "warmup"
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_warmup(bpm, _song_offset_s, _input_offset_ms):
		return
	_load_audio()
	_begin()


## Load the builtin fill chart (every kind including RUSH) at a BPM.
func start_fill(at_bpm: float) -> void:
	if game == null:
		return
	level_path = ""
	builtin = "fill"
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_fill(bpm, _song_offset_s, _input_offset_ms):
		return
	_load_audio()
	_begin()


## Load a level JSON (see levels/*.json). Audio comes from the level's
## `stems` (played in sync, faded by combo tier) or `audio` (single stream);
## neither = procedural click layers.
func start_level(path: String) -> void:
	if game == null:
		return
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		push_error("level not found: " + path)
		return
	var parsed = JSON.parse_string(text)
	if typeof(parsed) != TYPE_DICTIONARY:
		push_error("level json invalid: " + path)
		return
	_song_offset_s = float(parsed.get("song_offset_s", 0.0))
	if not game.load_level_json(text, _song_offset_s, _input_offset_ms):
		return
	level_path = path
	bpm = game.bpm()
	_load_audio()
	_begin()


## Streams for the run. The drum stem is the player's part: audible through the
## lead-in, muted from the first target on, replaced by kit hits. The remaining
## stems are the band and open by combo tier.
func _load_audio() -> void:
	var sync := AudioStreamSynchronized.new()
	var stems: Array = []
	var stem_paths: Array = game.stem_paths()
	var audio_path: String = str(game.audio_path())
	_drum_stem = int(game.drum_stem())
	if stem_paths.size() > 0:
		for p in stem_paths:
			var s = load(str(p))
			if s == null:
				push_error("stem not found: " + str(p))
				stems.append(AudioStreamWAV.new())
			else:
				stems.append(s)
	elif not audio_path.is_empty():
		var s = load(audio_path)
		if s == null:
			push_error("audio not found: " + audio_path)
		else:
			stems.append(s)
		_drum_stem = -1
	if stems.is_empty():
		# Click layers: [hats, accent] are the band, [pulse] is the drummer.
		var layers := ClickTrack.build_layers(bpm, CLICK_BEATS)
		stems = [layers[1], layers[2], layers[0]]
		_drum_stem = 2
	sync.stream_count = stems.size()
	for i in stems.size():
		sync.set_sync_stream(i, stems[i])
	player.stream = sync
	_stem_count = stems.size()
	_band.clear()
	for i in _stem_count:
		if i != _drum_stem:
			_band.append(i)
	var paths: Dictionary = game.kit_paths()
	kit = Kit.load_paths(paths) if not paths.is_empty() else Kit.build()
	_first_spawn_time = INF
	var n: int = game.target_count()
	if n > 0:
		for i in n:
			var st := float(game.target(i).spawn_time)
			if st < _first_spawn_time:
				_first_spawn_time = st
	_drum_live = true
	_apply_tier(0, true)


## Calibration run: 8 bars of click, no targets. Every click's distance to the
## nearest beat is collected; the median becomes the input offset. Measures the
## whole pipeline on the player's machine: mouse polling, frame, audio output.
func start_calibration() -> void:
	if game == null:
		return
	var json := JSON.stringify({
		"name": "calibrate", "bpm": bpm, "lead_in": 4.0,
		"sections": [{"pattern": "rest", "repeats": 8}],
	})
	_song_offset_s = 0.0
	if not game.load_level_json(json, _song_offset_s, 0.0):
		return
	_load_audio()
	calibrating = true
	_cal_samples.clear()
	_begin()


func _finish_calibration() -> void:
	calibrating = false
	_cal_samples.sort()
	var n := _cal_samples.size()
	var median := 0.0
	if n > 0:
		median = _cal_samples[n / 2] if n % 2 == 1 else (_cal_samples[n / 2 - 1] + _cal_samples[n / 2]) * 0.5
	set_input_offset_ms(-median)
	calibrated.emit(_input_offset_ms)
	restart()


func _begin() -> void:
	player.stop()
	_stream_started = false
	running = true
	tier = 0
	_frame_usec = Time.get_ticks_usec()
	game.start(-PREROLL_S)
	run_started.emit(bpm)


func restart() -> void:
	calibrating = false
	if not level_path.is_empty():
		start_level(level_path)
	elif builtin == "fill":
		start_fill(bpm)
	else:
		start_warmup(bpm)


func set_input_offset_ms(ms: float) -> void:
	_input_offset_ms = ms
	Settings.set_value("input", "offset_ms", ms)
	if game != null:
		game.set_input_offset_ms(ms)


func input_offset_ms() -> float:
	return _input_offset_ms


## Stream position corrected for output latency, the way the conductor wants it.
func audio_time() -> float:
	return player.get_playback_position() + AudioServer.get_time_since_last_mix() - AudioServer.get_output_latency()


## Song time at this instant: the model's time at the last update plus the
## wall clock since. Used to timestamp clicks between frames.
func input_song_time() -> float:
	if game == null:
		return 0.0
	return game.song_time() + float(Time.get_ticks_usec() - _frame_usec) / 1_000_000.0


func chart_time_now() -> float:
	return game.chart_time(game.song_time()) if game != null else 0.0


func beat_phase() -> float:
	return game.beat_phase() if game != null else 0.0


func _process(delta: float) -> void:
	if not running or game == null:
		return
	var has_audio := player.playing
	var events: Array = game.update(delta, has_audio, audio_time() if has_audio else 0.0)
	_frame_usec = Time.get_ticks_usec()
	if not _stream_started and game.song_time() >= 0.0:
		player.play(maxf(0.0, game.song_time()))
		_stream_started = true
	for e in events:
		_route(e)
	if _drum_live and game.chart_time(game.song_time()) >= _first_spawn_time:
		_set_drum(false)
	if game.is_finished():
		running = false
		player.stop()
		if calibrating:
			_finish_calibration()
		else:
			var report: Dictionary = game.report()
			last_replay_path = save_replay()
			run_finished.emit(report)


func _route(e: Dictionary) -> void:
	match e.get("type", ""):
		"tick":
			tick.emit(e.index, e.beat, e.sub)
		"beat":
			beat.emit(e.index, e.in_bar)
		"bar":
			bar.emit(e.index)
		"spawn":
			target_spawned.emit(e.target, game.target(e.target))
		"hit":
			target_hit.emit(e.target, e)
			if e.on_beat:
				play_kit(str(e.kind), float(KIT_DB.get(e.timing, -7.0)))
				_update_tier()
			else:
				play_kit("DEAD", DEAD_DB)
				_break()
		"whiff":
			whiff.emit(e)
			if calibrating:
				play_kit("FLICK", 0.0)
				_cal_samples.append(float(e.delta_ms))
				if _cal_samples.size() >= 16:
					_finish_calibration()
			else:
				play_kit("DEAD", DEAD_DB)
				_break()
		"expired":
			target_expired.emit(e.target)
			_break()
		"track_done":
			track_done.emit(e.target, e)
			if float(e.ratio) >= 0.5:
				play_kit("TRACK", linear_to_db(clampf(float(e.ratio), 0.3, 1.0)))
				_update_tier()
			else:
				_break()
		"ignored":
			pass


## A click: raw song time measured at the input event, the target id the ray
## hit (-1 for none) and the ray's distance from the target centre in metres.
func fire(raw_song_time: float, target_id: int, offset_m: float) -> Dictionary:
	if game == null or not running:
		return {}
	var v: Dictionary = game.fire(raw_song_time, target_id, offset_m)
	_route(v)
	return v


func track_sample(target_id: int, on_target: bool) -> void:
	if game != null and running:
		game.track_sample(target_id, on_target)


# ---------------------------------------------------------------- stems

func _update_tier() -> void:
	var combo: int = game.combo()
	var t := 0
	for i in TIER_COMBOS.size():
		if combo >= TIER_COMBOS[i]:
			t = i
	if t != tier:
		_apply_tier(t, false)


func _break() -> void:
	combo_broken.emit()
	if tier != 0:
		_apply_tier(0, false)
	_duck()


## Tier t opens band stems 0..t; with fewer band stems than tiers, the top
## tiers brighten the last one. Band stem 0 always plays. The drum stem is not
## part of the band: it follows `_set_drum`.
func _apply_tier(t: int, immediate: bool) -> void:
	tier = t
	tier_changed.emit(t)
	var sync := player.stream as AudioStreamSynchronized
	if sync == null:
		return
	if immediate and _duck_tween != null and _duck_tween.is_valid():
		_duck_tween.kill()
	for j in _band.size():
		var db := 0.0 if j <= t else MUTED_DB
		if j == _band.size() - 1 and t >= _band.size():
			db = 2.0
		sync.set_sync_stream_volume(_band[j], db)
	if _drum_stem >= 0:
		sync.set_sync_stream_volume(_drum_stem, 0.0 if _drum_live else MUTED_DB)


func _set_drum(live: bool) -> void:
	_drum_live = live
	var sync := player.stream as AudioStreamSynchronized
	if sync != null and _drum_stem >= 0:
		sync.set_sync_stream_volume(_drum_stem, 0.0 if live else MUTED_DB)


## One kit hit. The player's drum part is made of these.
func play_kit(kind: String, db: float) -> void:
	var sample = kit.get(kind)
	if sample == null:
		return
	if not kit_player.playing:
		kit_player.play()
	var pb := kit_player.get_stream_playback() as AudioStreamPlaybackPolyphonic
	if pb != null:
		pb.play_stream(sample, 0.0, db, 1.0)


func _duck() -> void:
	var sync := player.stream as AudioStreamSynchronized
	if sync == null or _band.is_empty():
		return
	if _duck_tween != null and _duck_tween.is_valid():
		_duck_tween.kill()
	var base: int = _band[0]
	sync.set_sync_stream_volume(base, DUCK_DB)
	_duck_tween = create_tween()
	_duck_tween.tween_method(func(db: float) -> void: sync.set_sync_stream_volume(base, db), DUCK_DB, 0.0, DUCK_S)


# ---------------------------------------------------------------- replays

func save_replay() -> String:
	if game == null:
		return ""
	var json: String = game.replay_json()
	if json.is_empty():
		return ""
	var user_dir := DirAccess.open("user://")
	if user_dir != null:
		user_dir.make_dir_recursive("replays")
	var stamp := Time.get_datetime_string_from_system(false, true).replace(":", "-")
	var path := "%s/%s_%s.json" % [REPLAY_DIR, str(game.level_name()), stamp]
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		push_warning("replay: could not write " + path)
		return ""
	f.store_string(json)
	f.close()
	return path


## Replay a saved run through the model and return its report.
func replay_report(path: String) -> Dictionary:
	if game == null:
		return {}
	var json := FileAccess.get_file_as_string(path)
	if json.is_empty():
		push_error("replay not found: " + path)
		return {}
	return game.replay_report(json)
