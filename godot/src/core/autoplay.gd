extends Node
## Autoplay: aims the real player at each target and fires on its beat through
## the same input path a human uses (timestamp, raycast, offset). Used by the
## headless smoke test (`--smoke --autoplay`) to prove the whole glue chain,
## and as an attract/demo mode. It plays perfectly on purpose: any miss is a
## bug in the scene, not the bot.

var player: Node = null
var spawner: Node = null
var _fired: Dictionary = {}
var _spans: Array = []
var _stands: Dictionary = {}


func _ready() -> void:
	player = get_parent().get_node_or_null("Player")
	spawner = get_parent().get_node_or_null("Spawner")
	DB.run_started.connect(_on_started)
	DB.restarted.connect(func(_seg: int, _t: float) -> void: _fired.clear())


func _on_started(_bpm: float) -> void:
	_fired.clear()
	_spans = DB.game.spans()
	_stands.clear()
	for r in DB.game.rooms():
		_stands[r.name] = r.stand


## Stand on the current encounter's stand point; the bot does not walk.
func _place() -> void:
	var beat: float = DB.game.song_beat()
	for sp in _spans:
		if sp.kind == "encounter" and beat >= float(sp.start_beat) - 4.0 and beat < float(sp.end_beat):
			var stand: Vector3 = _stands.get(sp.room, Vector3.ZERO)
			if player.global_position.distance_to(stand) > 0.5:
				player.global_position = stand + Vector3(0, 0.05, 0)
				player.velocity = Vector3.ZERO
				player.force_update_transform()
			return


func _process(_delta: float) -> void:
	if player == null or spawner == null or not DB.running or DB.game == null:
		return
	_place()
	var ct := DB.chart_time_now()
	var tracking := false
	for id in spawner.live:
		var node = spawner.live[id]
		if node.kind == "TRACK":
			if ct >= node.hit_time and ct <= node.end_time:
				_aim_at(node.weak_position())
				tracking = true
		elif not _fired.has(id) and ct >= node.hit_time:
			_aim_at(node.weak_position())
			_fired[id] = true
			player._fire(DB.input_song_time())
			return
	player.firing = tracking


func _aim_at(target: Vector3) -> void:
	var cam: Camera3D = player.cam
	var dir := (target - cam.global_position).normalized()
	player.yaw = atan2(-dir.x, -dir.z)
	player.pitch = asin(clampf(dir.y, -1.0, 1.0))
	player.rotation.y = player.yaw
	player.head.rotation.x = player.pitch
	# Transforms propagate on the next frame; force the camera's global
	# transform now so the raycast that follows uses the new aim.
	player.force_update_transform()
	player.head.force_update_transform()
	cam.force_update_transform()
