extends Node
## Watching a replay: the driver feeds the recorded inputs to the model; this
## node moves the camera. With a recorded view track (replays saved by this
## build) the camera follows the player's own path, interpolated per frame.
## Without one (older replays) it stands at the current room's stand point
## and snaps onto each shot's target as the shot lands.

var view: Array = []
var index := 0
var player: Node = null
var spawner: Node = null
var _stands: Dictionary = {}


func _ready() -> void:
	player = get_parent().get_node_or_null("Player")
	spawner = get_parent().get_node_or_null("Spawner")
	DB.replay_started.connect(_on_replay_started)
	DB.run_started.connect(func(_bpm: float) -> void:
		if not DB.watching:
			view = []
		_stands.clear()
		if DB.game != null:
			for r in DB.game.rooms():
				_stands[r.name] = r.stand)
	DB.target_hit.connect(_on_hit)
	DB.target_spawned.connect(_on_spawn)


func _on_replay_started(_inputs: Array, replay_view: Array) -> void:
	view = replay_view
	index = 0


func _process(_delta: float) -> void:
	if not DB.watching or DB.game == null or not DB.running or player == null or view.is_empty():
		return
	var ct := DB.chart_time_now()
	# Samples are in recording order; after a recorded restart the times go
	# back, and so does the model's clock, so a sequential cursor stays right.
	while index + 1 < view.size() and float(view[index + 1][0]) <= ct:
		index += 1
	var a: Array = view[index]
	var b: Array = view[mini(index + 1, view.size() - 1)]
	var ta := float(a[0])
	var tb := float(b[0])
	var k := 0.0 if tb <= ta else clampf((ct - ta) / (tb - ta), 0.0, 1.0)
	var pos := Vector3(float(a[1]), float(a[2]), float(a[3])).lerp(Vector3(float(b[1]), float(b[2]), float(b[3])), k)
	var yaw := lerp_angle(float(a[4]), float(b[4]), k)
	var pitch := lerpf(float(a[5]), float(b[5]), k)
	player.global_position = pos
	player.velocity = Vector3.ZERO
	player.set_view(yaw, pitch)


## Old replays without a view track: stand where the encounter is played from.
func _on_spawn(_id: int, info: Dictionary) -> void:
	if not DB.watching or not view.is_empty() or player == null:
		return
	var stand: Vector3 = _stands.get(info.room, Vector3.ZERO)
	if player.global_position.distance_to(stand) > 0.5:
		player.global_position = stand
		player.velocity = Vector3.ZERO


func _on_hit(id: int, _v: Dictionary) -> void:
	if not DB.watching or not view.is_empty() or player == null or spawner == null:
		return
	if spawner.live.has(id):
		var target: Vector3 = spawner.live[id].weak_position()
		var cam: Camera3D = player.cam
		var dir: Vector3 = (target - cam.global_position).normalized()
		player.set_view(atan2(-dir.x, -dir.z), asin(clampf(dir.y, -1.0, 1.0)))
