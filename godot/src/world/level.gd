extends Node3D
## Level builder: rooms, corridors and doors from the compiled chart. Every
## mesh is a greybox block (ASSETS.register.md [ENV-*]); the layout, the door
## timing and the start pad are real.

const FLOOR := Palette.CONCRETE
const WALL := Palette.STEEL
const BLOCK := Palette.BLOCK
const DOOR := Palette.VIOLET
const NEON_PINK := Palette.MAGENTA
const NEON_CYAN := Palette.CYAN
const WALL_T := 0.6
const DOOR_W := 4.0
const DOOR_H := 3.2

var doors: Array[StaticBody3D] = []
var _door_open_times: Array[float] = []
var _rooms: Array = []
var _spans: Array = []
var _built_for := ""
var _env: Environment
var _strips: Array[ShaderMaterial] = []
## Running set pieces: {kind, t, bars, node, room, param}.
var _active: Array[Dictionary] = []
var _water: Dictionary = {}


func _ready() -> void:
	_environment()
	DB.run_started.connect(_on_run_started)
	DB.cue.connect(_on_cue)
	DB.restarted.connect(func(_seg: int, _t: float) -> void: _clear_cues())


func _on_run_started(_bpm: float) -> void:
	var key := "%s@%s" % [str(DB.game.level_name()), str(DB.game.chart_fingerprint())]
	_clear_cues()
	_lights(0)
	if key == _built_for:
		for d in doors:
			d.position.y = 0.0
		return
	_built_for = key
	for c in get_children():
		if c.name.begins_with("Built"):
			c.queue_free()
	_strips.clear()
	_water.clear()
	_build()


## Rooms in path order; corridors join each room's exit (-Z face) to the next
## room's entrance (+Z face). The start pad sits at z = 0 and the first
## corridor leads from it to the first room.
func _build() -> void:
	var root := Node3D.new()
	root.name = "Built"
	add_child(root)
	_rooms = DB.game.rooms()
	_spans = DB.game.spans()
	doors.clear()
	_door_open_times.clear()
	var prev_exit_z := 0.0
	var prev_half_w := 3.0
	_pad(root, Vector3(0, 0, 4), 8.0)
	for i in _rooms.size():
		var r: Dictionary = _rooms[i]
		var o: Vector3 = r.origin
		var sz: Vector3 = r.size
		var entrance_z := o.z + sz.z * 0.5
		var exit_z := o.z - sz.z * 0.5
		_corridor(root, prev_exit_z, entrance_z, minf(prev_half_w, 3.0))
		_room(root, r, i)
		prev_exit_z = exit_z
		prev_half_w = sz.x * 0.5
	_corridor(root, prev_exit_z, prev_exit_z - 12.0, 3.0)
	_box(root, Vector3(0, 3.0, prev_exit_z - 12.3), Vector3(8, 6, WALL_T), WALL)
	# Door schedule: the door out of encounter k opens when the next traversal starts.
	for s in _spans:
		if s.kind == "traversal":
			_door_open_times.append(float(s.start_time))


func _room(root: Node3D, r: Dictionary, index: int) -> void:
	var o: Vector3 = r.origin
	var sz: Vector3 = r.size
	var hw := sz.x * 0.5
	var hd := sz.z * 0.5
	_box(root, o + Vector3(0, -0.5, 0), Vector3(sz.x, 1, sz.z), FLOOR)
	_box(root, o + Vector3(0, sz.y + 0.5, 0), Vector3(sz.x, 1, sz.z), WALL)
	_box(root, o + Vector3(-hw - WALL_T * 0.5, sz.y * 0.5, 0), Vector3(WALL_T, sz.y, sz.z), WALL)
	_box(root, o + Vector3(hw + WALL_T * 0.5, sz.y * 0.5, 0), Vector3(WALL_T, sz.y, sz.z), WALL)
	# Entrance (+Z) and exit (-Z) faces with a door gap in the middle.
	for face in [1.0, -1.0]:
		var z := o.z + face * (hd + WALL_T * 0.5)
		var side_w := (sz.x - DOOR_W) * 0.5
		_box(root, Vector3(o.x - DOOR_W * 0.5 - side_w * 0.5, sz.y * 0.5, z), Vector3(side_w, sz.y, WALL_T), WALL)
		_box(root, Vector3(o.x + DOOR_W * 0.5 + side_w * 0.5, sz.y * 0.5, z), Vector3(side_w, sz.y, WALL_T), WALL)
		_box(root, Vector3(o.x, DOOR_H + (sz.y - DOOR_H) * 0.5, z), Vector3(DOOR_W, sz.y - DOOR_H, WALL_T), WALL)
		if face < 0.0:
			var door := _box(root, Vector3(o.x, DOOR_H * 0.5, z), Vector3(DOOR_W, DOOR_H, WALL_T * 0.6), DOOR)
			door.name = "Door%d" % index
			doors.append(door)
	for b in r.blocks:
		_box(root, b.pos, b.size, BLOCK)
	if str(r.name) == "underpass":
		# Standing water: a flat translucent sheet the "flood" cue raises.
		var w := MeshInstance3D.new()
		var pm := BoxMesh.new()
		pm.size = Vector3(sz.x, 0.08, sz.z)
		w.mesh = pm
		w.position = o + Vector3(0, 0.12, 0)
		w.material_override = Cel.material(Color(Palette.CYAN, 0.35), 1.2, Palette.CYAN, false)
		root.add_child(w)
		_water = {"node": w, "base": 0.12, "room": str(r.name)}
	var stand: Vector3 = r.stand
	_strip(root, stand + Vector3(0, 0.02, 0), Vector3(1.4, 0.04, 1.4), NEON_CYAN)
	_strip(root, Vector3(o.x, 0.02, o.z - hd + 0.4), Vector3(sz.x - 2.0, 0.04, 0.3), NEON_PINK, true)
	_strip(root, Vector3(o.x - hw + 0.3, sz.y - 0.3, o.z), Vector3(0.1, 0.1, sz.z - 2.0), NEON_PINK, true)
	_strip(root, Vector3(o.x + hw - 0.3, sz.y - 0.3, o.z), Vector3(0.1, 0.1, sz.z - 2.0), NEON_PINK, true)


func _corridor(root: Node3D, from_z: float, to_z: float, half_w: float) -> void:
	var length := from_z - to_z
	if length <= 0.0:
		return
	var mid := (from_z + to_z) * 0.5
	var w := half_w * 2.0
	_box(root, Vector3(0, -0.5, mid), Vector3(w, 1, length), FLOOR)
	_box(root, Vector3(-half_w - WALL_T * 0.5, 2.5, mid), Vector3(WALL_T, 5.0, length), WALL)
	_box(root, Vector3(half_w + WALL_T * 0.5, 2.5, mid), Vector3(WALL_T, 5.0, length), WALL)
	_box(root, Vector3(0, 5.5, mid), Vector3(w, 1, length), WALL)
	var n := int(length / 6.0)
	for i in n:
		var z := from_z - 3.0 - i * 6.0
		_strip(root, Vector3(-half_w + 0.2, 1.2, z), Vector3(0.1, 0.1, 1.6), NEON_CYAN)
		_strip(root, Vector3(half_w - 0.2, 1.2, z), Vector3(0.1, 0.1, 1.6), NEON_CYAN)


func _pad(root: Node3D, at: Vector3, size: float) -> void:
	_box(root, at + Vector3(0, -0.5, 0), Vector3(size, 1, size), FLOOR)
	_box(root, at + Vector3(0, 2.5, size * 0.5 + WALL_T * 0.5), Vector3(size, 5, WALL_T), WALL)
	_box(root, at + Vector3(-size * 0.5 - WALL_T * 0.5, 2.5, 0), Vector3(WALL_T, 5, size), WALL)
	_box(root, at + Vector3(size * 0.5 + WALL_T * 0.5, 2.5, 0), Vector3(WALL_T, 5, size), WALL)
	_strip(root, Vector3(0, 0.02, 0), Vector3(1.2, 0.04, 1.2), NEON_CYAN)


func _process(delta: float) -> void:
	if not DB.running or DB.game == null:
		return
	var ct := DB.chart_time_now()
	for i in doors.size():
		if i < _door_open_times.size() and ct >= _door_open_times[i] and doors[i].position.y < DOOR_H * 0.99:
			doors[i].position.y = minf(doors[i].position.y + delta * DOOR_H * 2.0, DOOR_H)
	_run_cues(delta)


# ---------------------------------------------------------------- set pieces

## Cues from the chart (level JSON `cues`): the model times them, this plays them.
##   train   a train crosses the platform's trench over `bars` bars
##   lights  key light scheme: 0 night, 1 chorus (magenta), 2 drop (cyan), 3 boss (red)
##   flood   the underpass water rises to `param` metres over `bars` bars
##   reveal  the boss rises (boss.gd)      shake  camera (player.gd)
func _on_cue(c: Dictionary) -> void:
	match str(c.kind):
		"train":
			var room := _room_by_name(str(c.room))
			if room.is_empty():
				return
			var o: Vector3 = room.origin
			var sz: Vector3 = room.size
			var t := MeshInstance3D.new()
			var bm := BoxMesh.new()
			bm.size = Vector3(26.0, 3.4, 2.8)
			t.mesh = bm
			t.material_override = Cel.material(Palette.STEEL, 0.0, Palette.STEEL)
			t.position = o + Vector3(-sz.x * 0.5 - 14.0, 1.7 + 0.4, -8.0)
			add_child(t)
			var lamp := MeshInstance3D.new()
			var lm := BoxMesh.new()
			lm.size = Vector3(26.0, 0.3, 0.2)
			lamp.mesh = lm
			lamp.position = Vector3(0, 0.9, 1.45)
			lamp.material_override = Cel.material(Palette.YELLOW, 3.0, Palette.YELLOW, false)
			t.add_child(lamp)
			_active.append({"kind": "train", "t": 0.0, "bars": maxf(float(c.bars), 1.0), "node": t, "from": t.position.x, "to": o.x + sz.x * 0.5 + 14.0})
		"lights":
			_lights(int(c.param))
		"flood":
			if _water.is_empty():
				return
			_active.append({"kind": "flood", "t": 0.0, "bars": maxf(float(c.bars), 0.25), "node": _water.node, "from": (_water.node as Node3D).position.y, "to": float(_water.base) + float(c.param)})
		_:
			pass


func _run_cues(delta: float) -> void:
	var spb := 60.0 / maxf(DB.bpm, 1.0)
	var i := _active.size() - 1
	while i >= 0:
		var a := _active[i]
		a.t += delta
		var u: float = clampf(float(a.t) / (float(a.bars) * 4.0 * spb), 0.0, 1.0)
		var node: Node3D = a.node
		match str(a.kind):
			"train":
				node.position.x = lerpf(float(a.from), float(a.to), u)
			"flood":
				node.position.y = lerpf(float(a.from), float(a.to), u * u * (3.0 - 2.0 * u))
		if u >= 1.0:
			if str(a.kind) == "train":
				node.queue_free()
			_active.remove_at(i)
		i -= 1


func _clear_cues() -> void:
	for a in _active:
		if str(a.kind) == "train":
			(a.node as Node).queue_free()
	_active.clear()
	if not _water.is_empty():
		(_water.node as Node3D).position.y = float(_water.base)


## Key light scheme for the room strips and the ambient.
func _lights(scheme: int) -> void:
	var key: Color
	var ambient: Color
	match scheme:
		1:
			key = Palette.MAGENTA
			ambient = Color(0.42, 0.22, 0.48)
		2:
			key = Palette.CYAN
			ambient = Color(0.22, 0.40, 0.52)
		3:
			key = Palette.RED
			ambient = Color(0.48, 0.20, 0.26)
		_:
			key = Palette.MAGENTA
			ambient = Color(0.35, 0.30, 0.50)
	if _env != null:
		_env.ambient_light_color = ambient
	for m in _strips:
		m.set_shader_parameter("albedo", key)
		m.set_shader_parameter("emission_color", key)


func _room_by_name(name: String) -> Dictionary:
	for r in _rooms:
		if str(r.name) == name:
			return r
	return {}


func _environment() -> void:
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Palette.INK
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.35, 0.30, 0.50)
	env.ambient_light_energy = 0.6
	env.glow_enabled = true
	env.glow_intensity = 0.6
	env.glow_bloom = 0.15
	_env = env
	var we := WorldEnvironment.new()
	we.environment = env
	add_child(we)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-48, 32, 0)
	sun.light_energy = 1.4
	sun.light_color = Color(1.0, 0.92, 0.85)
	sun.shadow_enabled = true
	add_child(sun)


func _box(root: Node3D, at: Vector3, size: Vector3, color: Color) -> StaticBody3D:
	var body := StaticBody3D.new()
	body.position = at
	var mesh := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = size
	mesh.mesh = bm
	mesh.material_override = Cel.material(color)
	body.add_child(mesh)
	var col := CollisionShape3D.new()
	var shape := BoxShape3D.new()
	shape.size = size
	col.shape = shape
	body.add_child(col)
	root.add_child(body)
	return body


func _strip(root: Node3D, at: Vector3, size: Vector3, color: Color, key_light: bool = false) -> void:
	var mesh := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = size
	mesh.mesh = bm
	mesh.position = at
	var m := Cel.material(color, 3.0, color, false)
	mesh.material_override = m
	if key_light:
		_strips.append(m)
	root.add_child(mesh)
