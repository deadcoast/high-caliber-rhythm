extends Node3D
## Turns the model's target events into enemies and feeds tracking samples
## back. Positions come from the model in world space (room-resolved).

const EnemyScript := preload("res://src/world/enemy.gd")

var live: Dictionary = {}
var tracks: Array[int] = []
var player: Node = null
var _stands: Dictionary = {}


func _ready() -> void:
	player = get_parent().get_node_or_null("Player")
	DB.target_spawned.connect(_on_spawn)
	DB.target_hit.connect(func(id: int, _v: Dictionary) -> void: _kill(id, "hit"))
	DB.target_expired.connect(func(id: int) -> void: _kill(id, "expired"))
	DB.track_done.connect(func(id: int, _v: Dictionary) -> void: _kill(id, "track"))
	DB.tick.connect(_on_tick)
	DB.run_started.connect(_clear)
	DB.restarted.connect(func(_seg: int, _t: float) -> void: _clear(DB.bpm))
	DB.marked.connect(func(id: int) -> void:
		if live.has(id):
			live[id].set_marked(true))
	DB.volley_miss.connect(func(_v: Dictionary) -> void:
		for id in live:
			live[id].set_marked(false))
	var ghost := get_parent().get_node_or_null("Ghost")
	if ghost != null:
		ghost.ghost_hit.connect(func(id: int, _v: Dictionary) -> void:
			if live.has(id):
				live[id].ghost_flash())


func _clear(_bpm: float) -> void:
	for id in live:
		live[id].queue_free()
	live.clear()
	tracks.clear()
	_stands.clear()
	for r in DB.game.rooms():
		_stands[r.name] = r.stand


func _on_spawn(id: int, info: Dictionary) -> void:
	var e := Node3D.new()
	e.set_script(EnemyScript)
	e.name = "E%d" % id
	add_child(e)
	e.global_position = DB.game.target_position_at(id, float(info.spawn_time))
	var stand: Vector3 = _stands.get(info.room, Vector3.ZERO)
	e.setup(id, info, stand + Vector3(0, 1.6, 0), "HD" in DB.mods)
	if e.boss_part:
		var boss := get_parent().get_node_or_null("Boss")
		if boss != null and boss.alive:
			e.tether_to(boss.anchor())
	live[id] = e
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
			node.global_position = DB.game.target_position_at(id, ct)
			node.look_at_stand()
		node.update_cue(ct)
