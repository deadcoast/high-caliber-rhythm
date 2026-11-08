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
signal damaged(amount: int, hp: int, target: int)
signal down(downs: int)
signal chipped()
signal restarted(segment: int, chart_time: float)
signal volley(verdict: Dictionary)
signal volley_miss(verdict: Dictionary)
signal marked(target: int)
signal dashed(on_beat: bool)
## A boss phase closed (cleared or lost) and a set piece's bar arrived.
signal phase_end(phase: int, kills: int, total: int, cleared: bool, last: bool)
signal cue(info: Dictionary)
## A replay is being watched: its recorded inputs and the recorded camera track
## ([chart_time, x, y, z, yaw, pitch] per tick; empty for old replays).
signal replay_started(inputs: Array, view: Array)
## Any Song: a file has been analysed and wants confirming.
signal song_analysed(path: String, name: String, analysis: Dictionary)

const PREROLL_S := 2.0
const CLICK_BEATS := 128
## Combo counts at which each band stem tier opens. Tier 0 is the base layer alone.
const TIER_COMBOS := [0, 10, 25, 50]
const DUCK_DB := -9.0
const DUCK_S := 0.35
const REPLAY_DIR := "user://replays"
const SONG_DIR := "user://songs"
## Kit gain by verdict. Off-beat plays DEAD.
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
var current_drill := ""
var drill_minutes := 2.0
var bpm := 128.0
## Mods label for the next load ("NM", "DT", "DTHR"...). Applied in _begin's load path.
var mods := "NM"
var tier := 0
var calibrating := false
var last_replay_path := ""
## A replay is being watched: player input is off, the recorded inputs drive the model.
var watching := false
## Any Song preview (song + grid click) is playing; no run.
var previewing := false
## Any Song analysis running on a worker thread.
var analysing := false
## --song: build and play straight from the analysis, no confirm screen.
var skip_confirm := false
## The daily's date while a daily run is on (its score posts to the daily board).
var daily_date := ""
var analysing_name := ""
## A second model instance for Any Song: analysis runs on a worker thread
## against this one, so the game instance is never touched off the main thread.
var analysis_game = null

var _watch_level_json := ""
var _replay_inputs: Array = []
var _replay_index := 0
var _advance_pending := false
var _view := PackedFloat32Array()
var _decoded: Dictionary = {}
var _preview_path := ""
var _preview_bpm := 128.0
var _preview_offset := 0.0
var _preview_from := 0.0
var _analysis_task := -1
var _analysis_path := ""
var _analysis_result: Dictionary = {}
var _analysis_pcm: Dictionary = {}

var _frame_usec: int = 0
var _headless := false
var _stream_started := false
var _song_offset_s := 0.0
var _input_offset_ms := 0.0
var _cal_samples: Array[float] = []
var _stem_count := 0
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
	_headless = DisplayServer.get_name() == "headless"
	if not ClassDB.class_exists("DbGame"):
		push_error("downbeat_core GDExtension not loaded. Run tools/build.ps1 (Windows) or tools/build.sh, then restart.")
		return
	game = ClassDB.instantiate("DbGame")
	analysis_game = ClassDB.instantiate("DbGame")


func _setup_input_map() -> void:
	var defs := {
		"fire": [_mouse(MOUSE_BUTTON_LEFT)],
		"move_forward": [_key(KEY_W)],
		"move_back": [_key(KEY_S)],
		"move_left": [_key(KEY_A)],
		"move_right": [_key(KEY_D)],
		"jump": [_key(KEY_SPACE)],
		"sprint": [_key(KEY_SHIFT)],
		"crouch": [_key(KEY_CTRL)],
		"restart": [_key(KEY_R)],
		"release_mouse": [_key(KEY_ESCAPE)],
		"calibrate": [_key(KEY_C)],
		"menu": [_key(KEY_TAB)],
		"volley": [_mouse(MOUSE_BUTTON_RIGHT)],
		"dash": [_key(KEY_Q)],
		"routine": [_key(KEY_D)],
		"any_song": [_key(KEY_O)],
		"minutes_up": [_key(KEY_EQUAL)],
		"minutes_down": [_key(KEY_MINUS)],
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


## Toggle one mod in the label for the next load (DT and HT exclude each other).
func toggle_mod(mod: String) -> void:
	var parts: Array[String] = []
	var i := 0
	while i + 1 < mods.length():
		parts.append(mods.substr(i, 2))
		i += 2
	parts.erase("NM")
	if mod in parts:
		parts.erase(mod)
	else:
		if mod == "DT":
			parts.erase("HT")
		if mod == "HT":
			parts.erase("DT")
		parts.append(mod)
	var out := ""
	for o in ["DT", "HT", "HR", "HD", "NF"]:
		if o in parts:
			out += o
	mods = out if not out.is_empty() else "NM"


## Start a builtin level by name at a BPM ("street", "street_hard", "warmup", "fill").
func start_builtin(name: String, at_bpm: float) -> void:
	match name:
		"street":
			start_street(at_bpm)
		"street_hard":
			start_street_hard(at_bpm)
		"fill":
			start_fill(at_bpm)
		_:
			start_warmup(at_bpm)


## Load the builtin warmup chart at a BPM, backed by the procedural click layers.
func start_warmup(at_bpm: float) -> void:
	if game == null:
		return
	game.set_mods(mods)
	watching = false
	level_path = ""
	current_drill = ""
	builtin = "warmup"
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_warmup(bpm, _song_offset_s, _input_offset_ms):
		return
	_load_audio()
	_begin()


## Load the act-one street level (street -> atrium -> platform) at a BPM.
func start_street(at_bpm: float) -> void:
	if game == null:
		return
	game.set_mods(mods)
	watching = false
	level_path = ""
	current_drill = ""
	builtin = "street"
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_street(bpm, _song_offset_s, _input_offset_ms):
		return
	_load_audio()
	_begin()


## Street, Hard.
func start_street_hard(at_bpm: float) -> void:
	if game == null:
		return
	game.set_mods(mods)
	watching = false
	level_path = ""
	current_drill = ""
	builtin = "street_hard"
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_street_hard(bpm, _song_offset_s, _input_offset_ms):
		return
	_load_audio()
	_begin()


## Load the builtin fill chart (every kind including RUSH) at a BPM.
func start_fill(at_bpm: float) -> void:
	if game == null:
		return
	game.set_mods(mods)
	watching = false
	level_path = ""
	current_drill = ""
	builtin = "fill"
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_fill(bpm, _song_offset_s, _input_offset_ms):
		return
	_load_audio()
	_begin()


## Load a Range drill at a BPM for about `minutes`.
func start_drill(name: String, at_bpm: float, minutes: float) -> void:
	if game == null:
		return
	game.set_mods(mods)
	watching = false
	level_path = ""
	builtin = "drill"
	current_drill = name
	drill_minutes = minutes
	bpm = at_bpm
	_song_offset_s = 0.0
	if not game.load_drill(name, bpm, minutes, _input_offset_ms):
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
	start_level_text(text, path)


## The daily: the server's level JSON with its mods, scored on the daily board.
func start_daily(text: String, daily_mods: String, date: String) -> void:
	mods = daily_mods
	start_level_text(text)
	daily_date = date


## Load a level from its JSON text. `path` is what `restart` reloads; empty
## keeps the text itself for the restart.
func start_level_text(text: String, path: String = "") -> void:
	if game == null:
		return
	game.set_mods(mods)
	var parsed = JSON.parse_string(text)
	if typeof(parsed) != TYPE_DICTIONARY:
		push_error("level json invalid: " + path)
		return
	_song_offset_s = float(parsed.get("song_offset_s", 0.0))
	if not game.load_level_json(text, _song_offset_s, _input_offset_ms):
		return
	watching = false
	level_path = path
	_watch_level_json = text if path.is_empty() else ""
	builtin = "level"
	current_drill = ""
	bpm = game.bpm()
	_load_audio()
	_begin()


## Streams for the run: `stems` (opened by combo tier) or `audio`; neither
## = the procedural click layers. Hit-confirm kit from `kit` or synthesised.
func _load_audio() -> void:
	var sync := AudioStreamSynchronized.new()
	var stems: Array = []
	var stem_paths: Array = game.stem_paths()
	var audio_path: String = str(game.audio_path())
	if stem_paths.size() > 0:
		for p in stem_paths:
			var s = load(str(p))
			if s == null:
				push_error("stem not found: " + str(p))
				stems.append(AudioStreamWAV.new())
			else:
				stems.append(s)
	elif not audio_path.is_empty():
		var s = song_stream(audio_path)
		if s != null:
			stems.append(s)
	if stems.is_empty():
		for s in ClickTrack.build_layers(bpm, CLICK_BEATS):
			stems.append(s)
	sync.stream_count = stems.size()
	for i in stems.size():
		sync.set_sync_stream(i, stems[i])
	player.stream = sync
	_stem_count = stems.size()
	var paths: Dictionary = game.kit_paths()
	kit = Kit.load_paths(paths) if not paths.is_empty() else Kit.build()
	_apply_tier(0, true)


## Calibration run: 8 bars of click, no targets. Every click's distance to the
## nearest beat is collected; the median becomes the input offset. Measures the
## whole pipeline on the player's machine: mouse polling, frame, audio output.
func start_calibration() -> void:
	if game == null:
		return
	game.set_mods("NM")
	var json := JSON.stringify({
		"name": "calibrate", "bpm": bpm, "lead_in": 4.0,
		"sections": [{"pattern": "rest", "repeats": 8}],
	})
	_song_offset_s = 0.0
	if not game.load_level_json(json, _song_offset_s, 0.0):
		return
	watching = false
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
	daily_date = ""
	stop_preview()
	player.stop()
	_stream_started = false
	running = true
	tier = 0
	_view.clear()
	_frame_usec = Time.get_ticks_usec()
	game.start(-PREROLL_S)
	run_started.emit(bpm)


## Stop whatever is playing (a run, a watch, a preview) without a report.
func stop_run() -> void:
	running = false
	calibrating = false
	stop_preview()
	player.stop()


## R: play again. Watching a replay, R plays its level yourself.
func restart() -> void:
	calibrating = false
	var date := daily_date
	if watching or builtin == "level":
		if not level_path.is_empty():
			start_level(level_path)
		elif not _watch_level_json.is_empty():
			var text := _watch_level_json
			start_level_text(text)
			daily_date = date
		return
	if not level_path.is_empty():
		start_level(level_path)
	elif builtin == "fill":
		start_fill(bpm)
	elif builtin == "street":
		start_street(bpm)
	elif builtin == "street_hard":
		start_street_hard(bpm)
	elif builtin == "drill":
		start_drill(current_drill, bpm, drill_minutes)
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
	if analysing:
		_poll_analysis()
	if not running or game == null:
		return
	# Headless (CI): the dummy audio driver has no clock; the conductor runs
	# open loop on frame time, which is what the tests exercise.
	var has_audio := player.playing and not _headless
	if watching:
		_feed_replay(delta)
	var events: Array = game.update(delta, has_audio, audio_time() if has_audio else 0.0)
	_frame_usec = Time.get_ticks_usec()
	if not _stream_started and game.song_time() >= 0.0:
		player.play(maxf(0.0, game.song_time()))
		_stream_started = true
	for e in events:
		_route(e)
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
		"damage":
			damaged.emit(int(e.amount), int(e.hp), int(e.target))
		"down":
			down.emit(int(e.downs))
		"volley":
			volley.emit(e)
			play_kit("VOLLEY", 0.0)
			_update_tier()
		"volley_miss":
			volley_miss.emit(e)
			play_kit("DEAD", DEAD_DB)
			_break()
		"volley":
			volley.emit(e)
			play_kit("VOLLEY", 0.0)
			_update_tier()
		"volley_miss":
			volley_miss.emit(e)
			play_kit("DEAD", DEAD_DB)
			_break()
		"restart":
			# The model rewound. Rewind the song with it; the pre-roll logic
			# restarts the stream when song time comes back past 0.
			player.stop()
			_stream_started = false
			_apply_tier(0, true)
			restarted.emit(int(e.segment), float(e.chart_time))
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
	if game == null or not running or watching:
		return {}
	var v: Dictionary = game.fire(raw_song_time, target_id, offset_m)
	_route(v)
	return v


## Downbeat volley: mark a weak point under the crosshair while the charge is full.
func mark(target_id: int) -> bool:
	if game == null or not running or watching:
		return false
	if game.mark(target_id):
		marked.emit(target_id)
		return true
	return false


## Release the volley; judged against the nearest bar line.
func release(raw_song_time: float) -> Dictionary:
	if game == null or not running or watching:
		return {}
	var v: Dictionary = game.release(raw_song_time)
	# Kills produced by the volley arrive as hit events on the next update;
	# the volley verdict itself routes now.
	_route(v)
	return v


## A shot that hit an enemy body: no judgement, no combo change, a dull confirm.
func chip(_raw_song_time: float) -> void:
	if not running or watching:
		return
	play_kit("DEAD", -12.0)
	chipped.emit()


func track_sample(target_id: int, on_target: bool) -> void:
	if game != null and running and not watching:
		game.track_sample(target_id, on_target)


## Watching: before each frame's update, every recorded input whose chart
## time falls inside this frame is applied, the judge first advanced to the
## input's recorded time exactly as `Replay.play` does (the song clock is
## left alone and catches up in the update). The input lands up to one frame
## early on the wall clock and exactly on time in the model, so spawns,
## expiries, downs and restarts fall where they fell live. Verified in the
## core tests at uneven frame rates in both directions.
func _feed_replay(delta: float) -> void:
	var horizon := chart_time_now() + delta * 1.25 + 0.002
	while _replay_index < _replay_inputs.size() and float(_replay_inputs[_replay_index].at) <= horizon:
		_replay_input(_replay_inputs[_replay_index])
		_replay_index += 1
		# A restart inside the input moved the clock back; re-read it.
		horizon = chart_time_now() + delta * 1.25 + 0.002


## A restart while watching: expected when it came from a recorded Advance;
## otherwise this frame crossed the expiry before the recorded Advance time
## and the model downed on its own, so the queued inputs up to that Advance
## belong to the attempt that just ended and are dropped.
func _on_watched_restart() -> void:
	if _advance_pending:
		_advance_pending = false
		return
	while _replay_index < _replay_inputs.size():
		var was_advance := str(_replay_inputs[_replay_index].type) == "advance"
		_replay_index += 1
		if was_advance:
			break


func _replay_input(inp: Dictionary) -> void:
	if str(inp.type) == "advance":
		_advance_pending = true
	game.advance_judge(float(inp.at))
	match str(inp.type):
		"shot":
			_route(game.fire(float(inp.raw_time), int(inp.target), float(inp.offset_m)))
		"track":
			game.track_sample(int(inp.target), bool(inp.on))
		"mark":
			if game.mark(int(inp.target)):
				marked.emit(int(inp.target))
		"release":
			_route(game.release(float(inp.raw_time)))
		"advance":
			# The seek above reproduces the down and the restart.
			pass


## The player's camera, sampled once per tick while playing live, so a replay
## can be watched from where it was played (and raced as a ghost later).
func record_view(pos: Vector3, yaw: float, pitch: float) -> void:
	if not running or watching or calibrating or game == null:
		return
	_view.append(float(chart_time_now()))
	_view.append(pos.x)
	_view.append(pos.y)
	_view.append(pos.z)
	_view.append(yaw)
	_view.append(pitch)


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


## Tier t opens stems 0..t; with fewer stems than tiers, the top tiers brighten
## the last one. Stem 0 always plays.
func _apply_tier(t: int, immediate: bool) -> void:
	tier = t
	tier_changed.emit(t)
	var sync := player.stream as AudioStreamSynchronized
	if sync == null:
		return
	if immediate and _duck_tween != null and _duck_tween.is_valid():
		_duck_tween.kill()
	for i in _stem_count:
		var db := 0.0 if i <= t else MUTED_DB
		if i == _stem_count - 1 and t >= _stem_count:
			db = 2.0
		sync.set_sync_stream_volume(i, db)


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
	if sync == null or _stem_count == 0:
		return
	if _duck_tween != null and _duck_tween.is_valid():
		_duck_tween.kill()
	sync.set_sync_stream_volume(0, DUCK_DB)
	_duck_tween = create_tween()
	_duck_tween.tween_method(func(db: float) -> void: sync.set_sync_stream_volume(0, db), DUCK_DB, 0.0, DUCK_S)


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
	# The view track rides along as one more top-level key. The core's replay
	# text is kept byte for byte (it re-scores exactly); the core ignores keys
	# it does not know.
	var body := json.strip_edges()
	if body.ends_with("}") and _view.size() >= 6:
		var parts := PackedStringArray()
		for i in range(0, _view.size(), 6):
			parts.append("[%.4f,%.3f,%.3f,%.3f,%.4f,%.4f]" % [_view[i], _view[i + 1], _view[i + 2], _view[i + 3], _view[i + 4], _view[i + 5]])
		body = body.substr(0, body.length() - 1) + ',"view":[' + ",".join(parts) + "]}"
	f.store_string(body)
	f.close()
	return path


## Watch a saved run: the model is rebuilt from the replay (level, mods,
## calibration), the song plays, and the recorded inputs are fed back at
## their recorded times by the ReplayPlayer node. No score is recorded.
func watch_replay(path: String) -> bool:
	if game == null:
		return false
	var json := FileAccess.get_file_as_string(path)
	if json.is_empty():
		push_error("replay not found: " + path)
		return false
	if not game.load_replay(json):
		return false
	var parsed = JSON.parse_string(json)
	var view: Array = []
	if typeof(parsed) == TYPE_DICTIONARY:
		view = parsed.get("view", [])
		_watch_level_json = JSON.stringify(parsed.get("level", {}), "", false, true)
	mods = str(game.mods())
	watching = true
	calibrating = false
	level_path = ""
	builtin = "level"
	current_drill = ""
	bpm = game.bpm()
	_song_offset_s = float(game.song_offset_s())
	_replay_inputs = game.replay_inputs()
	_replay_index = 0
	_advance_pending = false
	_load_audio()
	_begin()
	replay_started.emit(_replay_inputs, view)
	return true


# ---------------------------------------------------------------- any song

## Decode a song to a WAV stream through the core (any format symphonia
## reads: wav, flac, ogg, mp3, m4a). Resources under res:// load as usual.
## Decoded streams are cached for the session so a restart is instant.
func song_stream(path: String) -> AudioStream:
	if path.begins_with("res://"):
		var s = load(path)
		if s == null:
			push_error("audio not found: " + path)
		return s
	var os_path := ProjectSettings.globalize_path(path)
	if _decoded.has(os_path):
		return _decoded[os_path]
	var d: Dictionary = game.decode_file(os_path)
	if d.is_empty():
		return null
	var wav := _wav_from(d)
	_decoded[os_path] = wav
	return wav


## Any Song, step one: analyse a file on a worker thread (tempo, beat 0,
## energy per bar) and decode it for playback; the confirm screen takes the
## result from `song_analysed`.
func analyse_song(path: String) -> void:
	if game == null or analysing:
		return
	stop_run()
	var os_path := ProjectSettings.globalize_path(path)
	analysing = true
	analysing_name = os_path.get_file().get_basename()
	_analysis_path = os_path
	_analysis_result = {}
	_analysis_pcm = {}
	_analysis_task = WorkerThreadPool.add_task(_analyse_worker, true, "downbeat any song")


func _analyse_worker() -> void:
	_analysis_result = analysis_game.analyze_file(_analysis_path)
	if not _analysis_result.is_empty():
		_analysis_pcm = analysis_game.decode_file(_analysis_path)


func _poll_analysis() -> void:
	if _analysis_task < 0 or not WorkerThreadPool.is_task_completed(_analysis_task):
		return
	WorkerThreadPool.wait_for_task_completion(_analysis_task)
	_analysis_task = -1
	analysing = false
	if _analysis_result.is_empty():
		return
	if not _analysis_pcm.is_empty():
		_decoded[_analysis_path] = _wav_from(_analysis_pcm)
		_analysis_pcm = {}
	song_analysed.emit(_analysis_path, analysing_name, _analysis_result)


func _wav_from(d: Dictionary) -> AudioStreamWAV:
	var wav := AudioStreamWAV.new()
	wav.format = AudioStreamWAV.FORMAT_16_BITS
	wav.mix_rate = int(d.sample_rate)
	wav.stereo = int(d.channels) == 2
	wav.data = d.pcm
	return wav


## Play the song with a metronome rendered onto the proposed grid, both in
## one synchronised stream, from `from_s` (default: beat 0).
func preview_grid(path: String, at_bpm: float, offset_s: float, from_s: float = -1.0) -> void:
	var song := song_stream(path)
	if song == null:
		return
	var duration: float = song.get_length()
	var c: Dictionary = analysis_game.grid_click(at_bpm, offset_s, duration, 44100)
	var click := AudioStreamWAV.new()
	click.format = AudioStreamWAV.FORMAT_16_BITS
	click.mix_rate = int(c.sample_rate)
	click.stereo = false
	click.data = c.pcm
	var sync := AudioStreamSynchronized.new()
	sync.stream_count = 2
	sync.set_sync_stream(0, song)
	sync.set_sync_stream(1, click)
	sync.set_sync_stream_volume(1, -4.0)
	running = false
	player.stop()
	player.stream = sync
	_preview_path = path
	_preview_bpm = at_bpm
	_preview_offset = offset_s
	if from_s >= 0.0:
		_preview_from = from_s
	elif _preview_path != path or _preview_from <= 0.0:
		_preview_from = offset_s
	previewing = true
	player.play(clampf(_preview_from, 0.0, maxf(duration - 1.0, 0.0)))


## Jump the preview by whole bars (Space: +8).
func preview_seek_bars(bars: int) -> void:
	if not previewing:
		return
	var spb := 60.0 / _preview_bpm
	var pos := player.get_playback_position() + bars * 4.0 * spb
	# Land on a bar line of the grid.
	var k := round((pos - _preview_offset) / (4.0 * spb))
	preview_grid(_preview_path, _preview_bpm, _preview_offset, maxf(_preview_offset + k * 4.0 * spb, 0.0))


func stop_preview() -> void:
	if previewing:
		previewing = false
		player.stop()


## Any Song, step three: write the auto level next to the song's name under
## user://songs and play it. The level keeps the song's absolute path.
func build_any_song(path: String, name: String, at_bpm: float, offset_s: float, analysis: Dictionary) -> String:
	if game == null:
		return ""
	stop_preview()
	var os_path := ProjectSettings.globalize_path(path)
	var energy: Array[float] = []
	for v in analysis.get("energy", []):
		energy.append(float(v))
	var json: String = analysis_game.auto_level_json(name, os_path, at_bpm, offset_s, float(analysis.get("duration_s", 0.0)), energy)
	var user_dir := DirAccess.open("user://")
	if user_dir != null:
		user_dir.make_dir_recursive("songs")
	var slug := name.to_lower().validate_filename().replace(" ", "_")
	var out := "%s/%s_%d.json" % [SONG_DIR, slug, int(round(at_bpm))]
	var f := FileAccess.open(out, FileAccess.WRITE)
	if f == null:
		push_warning("any song: could not write " + out)
		start_level_text(json)
		return ""
	f.store_string(json)
	f.close()
	start_level(out)
	return out


## Replay a saved run through the model and return its report.
func replay_report(path: String) -> Dictionary:
	if game == null:
		return {}
	var json := FileAccess.get_file_as_string(path)
	if json.is_empty():
		push_error("replay not found: " + path)
		return {}
	return game.replay_report(json)
