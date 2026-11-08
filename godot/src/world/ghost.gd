extends Node
## Ghosts: race a replay. The ghost is a second model fed the replay's inputs
## in lockstep with the song; its kills flash on the enemies it killed, its
## aim is a hollow crosshair from the recorded view track, the HUD shows its
## score against yours. Armed from a board (the #1 replay) before a run;
## active only when the live chart and mods match the replay's.

signal ghost_armed(name: String, score: int)
signal ghost_hit(target: int, verdict: Dictionary)
signal ghost_restart()

var armed := false
var active := false
var ghost_name := ""
var ghost_score := 0
var fingerprint := ""
var mods := "NM"
var game = null
var feeder := GhostFeeder.new()
var view: Array = []
var view_index := 0
var restarts := 0
var _replay_json := ""


func _ready() -> void:
	DB.run_started.connect(_on_run_started)
	DB.run_finished.connect(func(_r: Dictionary) -> void: active = false)
	Online.replay_received.connect(func(_id: int, json: String, meta: Dictionary) -> void:
		arm(json, str(meta.get("name", "ghost")), int(meta.get("score", 0))))


## Load a replay as the ghost for the next run of its chart.
func arm(replay_json: String, name: String, score: int) -> bool:
	if DB.game == null:
		return false
	game = ClassDB.instantiate("DbGame")
	if not game.load_replay(replay_json):
		game = null
		armed = false
		return false
	_replay_json = replay_json
	ghost_name = name
	ghost_score = score
	fingerprint = str(game.chart_fingerprint())
	mods = str(game.mods())
	var parsed = JSON.parse_string(replay_json)
	view = parsed.get("view", []) if typeof(parsed) == TYPE_DICTIONARY else []
	armed = true
	ghost_armed.emit(ghost_name, ghost_score)
	return true


func disarm() -> void:
	armed = false
	active = false
	game = null


func _on_run_started(_bpm: float) -> void:
	active = false
	if not armed or game == null or DB.watching:
		return
	if str(DB.game.chart_fingerprint()) != fingerprint or str(DB.game.mods()) != mods:
		return
	# Rebuild the ghost model from the replay so a second run starts it fresh.
	game = ClassDB.instantiate("DbGame")
	if not game.load_replay(_replay_json):
		return
	feeder.setup(game, game.replay_inputs())
	view_index = 0
	restarts = 0
	game.start(-DB.PREROLL_S)
	active = true


func _process(delta: float) -> void:
	if not active or game == null or not DB.running:
		return
	feeder.feed(delta)
	# Locked to the song while the ghost is on the same timeline; after a
	# restart of its own it runs open loop (it fell behind).
	var has_audio: bool = DB.player.playing and restarts == 0
	var events: Array = game.update(delta, has_audio, DB.audio_time() if has_audio else 0.0)
	for e in events:
		match str(e.get("type", "")):
			"hit":
				ghost_hit.emit(int(e.target), e)
			"restart":
				restarts += 1
				feeder.on_restart()
				ghost_restart.emit()


func score() -> int:
	return int(game.score()) if active and game != null else 0


## Seconds the ghost's timeline is behind the live one (0 while locked).
func behind_s() -> float:
	if not active or game == null:
		return 0.0
	return maxf(DB.chart_time_now() - float(game.chart_time(game.song_time())), 0.0)


## The ghost's aim right now (yaw, pitch) from its view track, or empty.
func aim() -> Array:
	if not active or view.is_empty():
		return []
	var ct: float = game.chart_time(game.song_time())
	while view_index + 1 < view.size() and float(view[view_index + 1][0]) <= ct:
		view_index += 1
	var a: Array = view[view_index]
	var b: Array = view[mini(view_index + 1, view.size() - 1)]
	var ta := float(a[0])
	var tb := float(b[0])
	var k := 0.0 if tb <= ta else clampf((ct - ta) / (tb - ta), 0.0, 1.0)
	return [lerp_angle(float(a[4]), float(b[4]), k), lerpf(float(a[5]), float(b[5]), k)]
