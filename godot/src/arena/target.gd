extends Area3D
## One chart target. Spawned by the Spawner with its compiled info; the
## approach ring collapses onto the body exactly at hit_time so the timing cue
## is visual as well as audible. Body, ring and death VFX are placeholder
## primitives (ASSETS.register.md [TGT-*]).

const COLORS := {
	"FLICK": Color(1.0, 0.18, 0.70),
	"SWITCH": Color(1.0, 0.65, 0.10),
	"TRACK": Color(0.15, 0.95, 1.0),
	"MICRO": Color(0.65, 1.0, 0.25),
	"RUSH": Color(1.0, 0.95, 0.35),
}

var target_id := -1
var kind := "FLICK"
var radius := 0.45
var spawn_time := 0.0
var hit_time := 0.0
var end_time := 0.0

var _body: MeshInstance3D
var _ring: MeshInstance3D
var _mat: ShaderMaterial
var _dying := false
var _die_t := 0.0
var _die_mode := ""


func setup(id: int, info: Dictionary) -> void:
	target_id = id
	kind = info.kind
	radius = info.radius
	spawn_time = info.spawn_time
	hit_time = info.hit_time
	end_time = info.end_time
	add_to_group("targets")
	var color: Color = COLORS.get(kind, Color.WHITE)
	_mat = Cel.material(color, 0.8, color)
	_body = MeshInstance3D.new()
	var sm := SphereMesh.new()
	sm.radius = radius
	sm.height = radius * 2.0
	sm.radial_segments = 24
	sm.rings = 12
	_body.mesh = sm
	_body.material_override = _mat
	add_child(_body)
	var col := CollisionShape3D.new()
	var shape := SphereShape3D.new()
	shape.radius = radius
	col.shape = shape
	add_child(col)
	if kind != "TRACK":
		_ring = MeshInstance3D.new()
		var tm := TorusMesh.new()
		tm.inner_radius = 0.9
		tm.outer_radius = 1.0
		tm.rings = 32
		tm.ring_segments = 8
		_ring.mesh = tm
		_ring.material_override = Cel.material(color, 2.0, color, false)
		add_child(_ring)


func update_cue(chart_time: float) -> void:
	if _dying:
		return
	if _ring != null:
		var span := maxf(hit_time - spawn_time, 0.001)
		var u := clampf((chart_time - spawn_time) / span, 0.0, 1.0)
		var r := lerpf(radius * 3.2, radius * 1.05, u)
		_ring.scale = Vector3(r, r, r)
		var cam := get_viewport().get_camera_3d()
		if cam != null:
			# Torus lies in its local XZ plane; face that plane at the camera.
			_ring.look_at(cam.global_position, Vector3.UP)
			_ring.rotate_object_local(Vector3.RIGHT, PI * 0.5)
	elif kind == "TRACK":
		var active := chart_time >= hit_time and chart_time <= end_time
		_mat.set_shader_parameter("emission_strength", 2.5 if active else 0.6)


func die(mode: String) -> void:
	if _dying:
		return
	_dying = true
	_die_mode = mode
	_die_t = 0.0
	if _ring != null:
		_ring.visible = false
	collision_layer = 0
	monitorable = false
	remove_from_group("targets")
	if mode == "hit":
		_mat.set_shader_parameter("emission_strength", 6.0)
	elif mode == "expired":
		_mat.set_shader_parameter("emission_strength", 0.0)
		_mat.set_shader_parameter("albedo", Color(0.25, 0.22, 0.30))


func _process(delta: float) -> void:
	if not _dying:
		return
	_die_t += delta
	if _die_mode == "hit":
		var s := 1.0 + _die_t * 6.0
		_body.scale = Vector3(s, s, s)
		if _die_t > 0.12:
			queue_free()
	else:
		var s := maxf(1.0 - _die_t * 3.0, 0.0)
		_body.scale = Vector3(s, s, s)
		if _die_t > 0.35:
			queue_free()
