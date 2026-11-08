extends Node3D
## Turns the model's target events into scene nodes and feeds tracking
## samples back. The lane origin is this node's position; angles from the
## chart are measured from here, facing -Z.

const TargetScript := preload("res://src/arena/target.gd")

@export var lane_origin := Vector3(0, 1.6, 0)

var live: Dictionary = {}
var tracks: Array[int] = []
var player: Node = null


func _ready() -> void:
	player = get_parent().get_node_or_null("Player")
	DB.target_spawned.connect(_on_spawn)
	DB.target_hit.connect(func(id: int, _v: Dictionary) -> void: _kill(id, "hit"))
	DB.target_expired.connect(func(id: int) -> void: _kill(id, "expired"))
	DB.track_done.connect(func(id: int, _v: Dictionary) -> void: _kill(id, "track"))
	DB.tick.connect(_on_tick)
	DB.run_started.connect(_clear)


func _clear(_bpm: float) -> void:
	for id in live:
		live[id].queue_free()
	live.clear()
	tracks.clear()


func _on_spawn(id: int, info: Dictionary) -> void:
	var t := Area3D.new()
	t.set_script(TargetScript)
	t.name = "T%d" % id
	add_child(t)
	t.setup(id, info)
	t.global_position = lane_origin + DB.game.target_position_at(id, info.spawn_time)
	live[id] = t
	if info.kind == "TRACK":
		tracks.append(id)


func _kill(id: int, mode: String) -> void:
	if live.has(id):
		live[id].die(mode)
		live.erase(id)
	tracks.erase(id)


func _on_tick(_index: int, _beat: int, _sub: int) -> void:
	if player == null:
		return
	for id in tracks:
		var node: Node3D = live.get(id)
		if node == null:
			continue
		DB.track_sample(id, player.firing and player.is_on(node))


func _process(_delta: float) -> void:
	if not DB.running:
		return
	var ct := DB.chart_time_now()
	for id in live:
		var node = live[id]
		if node.kind == "TRACK":
			node.global_position = lane_origin + DB.game.target_position_at(id, ct)
		node.update_cue(ct)
