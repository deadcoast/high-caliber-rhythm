extends Node3D
## The boss: one body in the boss room, the weak points of every phase hang
## off it (enemy.gd boss parts). Built from the chart's phases on run start,
## revealed by the "reveal" cue (it rises from below the pad), reacts to
## every phase end: a cleared phase drops a plate off the body, a lost phase
## is its attack, the last cleared phase is the kill. Greybox silhouette
## (ASSETS.register.md [TGT-10]); the timing and the states are final.

const BODY := Color(0.20, 0.17, 0.30)

var alive := false
var room_origin := Vector3.ZERO
var rest_y := 7.0
var _rise := 1.0
var _rise_t := 0.0
var _rise_from := -14.0
var _hull: Node3D
var _plates: Array[MeshInstance3D] = []
var _core: MeshInstance3D
var _core_mat: ShaderMaterial
var _attack := 0.0
var _dying := false
var _die_t := 0.0
var _bob_phase := 0.0


func _ready() -> void:
	DB.run_started.connect(_on_run_started)
	DB.cue.connect(_on_cue)
	DB.phase_end.connect(_on_phase_end)
	DB.restarted.connect(func(_seg: int, _t: float) -> void:
		if alive:
			_attack = 0.0)


func _on_run_started(_bpm: float) -> void:
	for c in get_children():
		c.queue_free()
	_plates.clear()
	alive = false
	_dying = false
	if DB.game == null:
		return
	var phases: Array = DB.game.phases()
	if phases.is_empty():
		return
	var room_name := str(phases[0].room)
	for r in DB.game.rooms():
		if str(r.name) == room_name:
			room_origin = r.origin
			var sz: Vector3 = r.size
			rest_y = minf(sz.y - 3.0, 7.0)
			_build(sz)
			alive = true
			# Hidden below the pad until the reveal cue; shown at once when the chart has none.
			var has_reveal := false
			for c in DB.game.cues():
				if str(c.kind) == "reveal":
					has_reveal = true
			_rise = 0.0 if has_reveal else 1.0
			_place()
			return


func _build(room_size: Vector3) -> void:
	_hull = Node3D.new()
	add_child(_hull)
	# Production model (ASSETS.register.md [TGT-10]): res://assets/boss/crane.glb
	# with its origin at the hull centre replaces the greybox hull; the plates,
	# core and lamps below stay as the state carriers until the rig has them.
	var model_path := "res://assets/boss/crane.glb"
	if ResourceLoader.exists(model_path):
		var packed = load(model_path)
		if packed != null and packed is PackedScene:
			var model: Node3D = (packed as PackedScene).instantiate()
			_hull.add_child(model)
			if _adopt_rig(model):
				return
			_build_states(room_size, false)
			return
	_build_states(room_size, true)


## A rigged hull carries its own state nodes: plate_0..plate_5, core, lamp_l,
## lamp_r (MeshInstance3D). The core takes the engine's cel material so the
## pulse, the attack and the kill drive it. True when all were found.
func _adopt_rig(model: Node3D) -> bool:
	var plates: Array[MeshInstance3D] = []
	for i in 6:
		var p := model.find_child("plate_%d" % i, true, false)
		if p == null or not (p is MeshInstance3D):
			return false
		plates.append(p as MeshInstance3D)
	var core := model.find_child("core", true, false)
	if core == null or not (core is MeshInstance3D):
		return false
	_plates = plates
	_core = core as MeshInstance3D
	_core_mat = Cel.material(Palette.YELLOW, 2.0, Palette.YELLOW)
	_core.material_override = _core_mat
	return true


func _build_states(room_size: Vector3, greybox_hull: bool) -> void:
	if greybox_hull:
		_mesh(_hull, Vector3(0, 0, 0), Vector3(7.0, 2.2, 3.2), BODY)
		_mesh(_hull, Vector3(0, 1.8, 0.2), Vector3(3.0, 1.6, 2.2), BODY)
	# Six armour plates along the front edge: one drops per cleared phase.
	for i in 6:
		var x := -2.5 + i * 1.0
		var pm := _mesh(_hull, Vector3(x, -0.4, 1.7), Vector3(0.9, 1.4, 0.3), Palette.STEEL)
		_plates.append(pm)
	# The core: lit, pulses on the beat.
	_core_mat = Cel.material(Palette.YELLOW, 2.0, Palette.YELLOW)
	_core = _mesh(_hull, Vector3(0, 0.4, 1.8), Vector3(1.2, 1.2, 0.4), Palette.YELLOW)
	_core.material_override = _core_mat
	# Two arms out to the sides, lamps at the ends.
	if greybox_hull:
		_mesh(_hull, Vector3(-6.0, 0.6, 0), Vector3(5.0, 0.5, 0.5), BODY)
		_mesh(_hull, Vector3(6.0, 0.6, 0), Vector3(5.0, 0.5, 0.5), BODY)
	_mesh(_hull, Vector3(-8.4, 0.6, 0.4), Vector3(0.7, 0.7, 0.7), Palette.CYAN, 2.0)
	_mesh(_hull, Vector3(8.4, 0.6, 0.4), Vector3(0.7, 0.7, 0.7), Palette.CYAN, 2.0)
	# A cable up to the ceiling.
	_mesh(_hull, Vector3(0, room_size.y * 0.5 + 1.0, 0), Vector3(0.2, room_size.y, 0.2), Palette.INK_LINE)


func _mesh(parent: Node3D, at: Vector3, size: Vector3, color: Color, emission: float = 0.0) -> MeshInstance3D:
	var m := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = size
	m.mesh = bm
	m.position = at
	m.material_override = Cel.material(color, emission, color) if emission > 0.0 else Cel.material(color)
	parent.add_child(m)
	return m


func _place() -> void:
	# Hangs at the far end of the room, in front of the crane base.
	var y := lerpf(_rise_from, rest_y, _rise)
	global_position = room_origin + Vector3(0, y, -12.0)


## Where boss parts tether to.
func anchor() -> Vector3:
	return global_position + Vector3(0, 0.4, 1.8)


func _on_cue(c: Dictionary) -> void:
	if not alive or str(c.kind) != "reveal":
		return
	_rise_t = 0.0
	_rise = 0.0


func _on_phase_end(phase: int, _kills: int, _total: int, cleared: bool, last: bool) -> void:
	if not alive:
		return
	if cleared:
		var k := mini(phase * 2, _plates.size() - 2)
		for i in [k, k + 1]:
			if i < _plates.size():
				_plates[i].visible = false
		_core_mat.set_shader_parameter("emission_strength", 6.0)
		if last:
			_dying = true
			_die_t = 0.0
	else:
		_attack = 1.0


func _process(delta: float) -> void:
	if not alive:
		return
	if _rise < 1.0:
		_rise_t += delta
		var bars := 2.0
		var spb := 60.0 / maxf(DB.bpm, 1.0)
		_rise = clampf(_rise_t / (bars * 4.0 * spb), 0.0, 1.0)
		_rise = _rise * _rise * (3.0 - 2.0 * _rise)
	_place()
	_bob_phase = DB.beat_phase()
	_hull.position.y = -0.08 * cos(_bob_phase * TAU)
	var pulse := 2.0 + 2.5 * (1.0 - _bob_phase)
	if _attack > 0.0:
		_attack = maxf(_attack - delta * 1.5, 0.0)
		_core_mat.set_shader_parameter("emission_strength", 8.0 * _attack)
		_core_mat.set_shader_parameter("emission_color", Palette.RED)
		_hull.position.z = -0.6 * _attack
	elif _dying:
		_die_t += delta
		var s := maxf(1.0 - _die_t * 0.8, 0.0)
		_hull.scale = Vector3(1.0 + _die_t * 0.3, s, 1.0 + _die_t * 0.3)
		_core_mat.set_shader_parameter("emission_strength", 10.0 * s)
		if _die_t > 1.4:
			alive = false
			_hull.visible = false
	else:
		_core_mat.set_shader_parameter("emission_color", Palette.YELLOW)
		_core_mat.set_shader_parameter("emission_strength", pulse)
