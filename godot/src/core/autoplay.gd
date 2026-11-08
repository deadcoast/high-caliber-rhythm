extends Node
## Autoplay: aims the real player at each target and fires on its beat through
## the same input path a human uses (timestamp, raycast, offset). Used by the
## headless smoke test (`--smoke --autoplay`) to prove the whole glue chain,
## and as an attract/demo mode. It plays perfectly on purpose: any miss is a
## bug in the scene, not the bot.

var player: Node = null
var spawner: Node = null
var _fired: Dictionary = {}


func _ready() -> void:
	player = get_parent().get_node_or_null("Player")
	spawner = get_parent().get_node_or_null("Spawner")
	DB.run_started.connect(func(_bpm: float) -> void: _fired.clear())


func _process(_delta: float) -> void:
	if player == null or spawner == null or not DB.running or DB.game == null:
		return
	var ct := DB.chart_time_now()
	var tracking := false
	for id in spawner.live:
		var node = spawner.live[id]
		if node.kind == "TRACK":
			if ct >= node.hit_time and ct <= node.end_time:
				_aim_at(node.global_position)
				tracking = true
		elif not _fired.has(id) and ct >= node.hit_time:
			_aim_at(node.global_position)
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
