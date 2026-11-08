extends Node3D
## One enemy: a body (chip damage, never judged) and a weak point (the judged
## target) with an approach ring that closes on the hit beat. Body shapes,
## ring and deaths are greybox (ASSETS.register.md [TGT-*]); the exposure,
## telegraph and fire timing are the real mechanics.

const BODY_COLOR := Color(0.24, 0.22, 0.32)
## Production models (ASSETS.register.md [TGT-01..05], art/BRIEFS.enemies.txt):
## `res://assets/enemies/<enemy>.glb` with its origin at the weak-point centre
## and -Z facing the player replaces the primitive body the moment it exists.
const MODEL_DIR := "res://assets/enemies/"

var target_id := -1
var kind := "FLICK"
var enemy := "DRONE"
var radius := 0.45
var spawn_time := 0.0
var hit_time := 0.0
var end_time := 0.0
var stand := Vector3.ZERO

var weak: Area3D
var body: StaticBody3D
var _body_mesh: MeshInstance3D
var _weak_mesh: MeshInstance3D
var _ring: MeshInstance3D
var _mat: ShaderMaterial
var _flash: MeshInstance3D
var _dying := false
var _die_t := 0.0
var _die_mode := ""
var _born := 0.0


var _marked := false
var _hidden := false
## Boss weak point: hangs off the boss body instead of carrying its own.
var boss_part := false
var _tether: MeshInstance3D


func setup(id: int, info: Dictionary, stand_point: Vector3, hidden: bool = false) -> void:
	_hidden = hidden
	boss_part = int(info.get("phase", -1)) >= 0
	target_id = id
	kind = info.kind
	enemy = info.enemy
	radius = info.radius
	spawn_time = info.spawn_time
	hit_time = info.hit_time
	end_time = info.end_time
	stand = stand_point
	var color: Color = Palette.KIND.get(kind, Color.WHITE)
	_mat = Cel.material(color, 0.8, color)
	# Body: silhouette by enemy type, below/around the weak point.
	body = StaticBody3D.new()
	body.add_to_group("enemy_body")
	_body_mesh = MeshInstance3D.new()
	var shape: Shape3D
	match enemy:
		"DRONE":
			var bm := BoxMesh.new()
			bm.size = Vector3(0.9, 0.5, 0.9)
			_body_mesh.mesh = bm
			var bs := BoxShape3D.new()
			bs.size = bm.size
			shape = bs
			body.position = Vector3(0, -0.45, 0)
		"GUNNER":
			var cm := CapsuleMesh.new()
			cm.radius = 0.35
			cm.height = 1.7
			_body_mesh.mesh = cm
			var cs := CapsuleShape3D.new()
			cs.radius = 0.35
			cs.height = 1.7
			shape = cs
			body.position = Vector3(0, -0.95, 0)
		"STRIDER":
			var cm2 := CapsuleMesh.new()
			cm2.radius = 0.45
			cm2.height = 2.4
			_body_mesh.mesh = cm2
			_body_mesh.rotation_degrees.z = 90.0
			var cs2 := CapsuleShape3D.new()
			cs2.radius = 0.45
			cs2.height = 2.4
			shape = cs2
			body.position = Vector3(0, -0.8, 0)
			body.rotation_degrees.z = 90.0
		"SWARM":
			var sm := SphereMesh.new()
			sm.radius = 0.28
			sm.height = 0.56
			_body_mesh.mesh = sm
			var ss := SphereShape3D.new()
			ss.radius = 0.28
			shape = ss
			body.position = Vector3(0, -0.4, 0)
		_:
			var hb := BoxMesh.new()
			hb.size = Vector3(1.8, 2.2, 1.2)
			_body_mesh.mesh = hb
			var hs := BoxShape3D.new()
			hs.size = hb.size
			shape = hs
			body.position = Vector3(0, -1.3, 0)
	_body_mesh.material_override = Cel.material(BODY_COLOR)
	body.add_child(_body_mesh)
	var bcol := CollisionShape3D.new()
	bcol.shape = shape
	body.add_child(bcol)
	add_child(body)
	var model_path := MODEL_DIR + enemy.to_lower() + ".glb"
	if ResourceLoader.exists(model_path):
		var packed = load(model_path)
		if packed != null and packed is PackedScene:
			var model: Node3D = (packed as PackedScene).instantiate()
			# The model is authored around the weak point; the body node sits
			# below it, so the model goes on this node, not the body.
			add_child(model)
			_body_mesh.visible = false
	if boss_part:
		# The boss is the body: this part is a pod, a lamp, a plate on it.
		body.visible = false
		body.collision_layer = 0
		_body_mesh.scale = Vector3.ZERO
	# Weak point: the judged sphere.
	weak = Area3D.new()
	weak.add_to_group("targets")
	weak.set_meta("target_id", id)
	_weak_mesh = MeshInstance3D.new()
	var wm := SphereMesh.new()
	wm.radius = radius
	wm.height = radius * 2.0
	wm.radial_segments = 24
	wm.rings = 12
	_weak_mesh.mesh = wm
	_weak_mesh.material_override = _mat
	weak.add_child(_weak_mesh)
	var wcol := CollisionShape3D.new()
	var ws := SphereShape3D.new()
	ws.radius = radius
	wcol.shape = ws
	weak.add_child(wcol)
	add_child(weak)
	if kind != "TRACK" and not _hidden:
		_ring = MeshInstance3D.new()
		var tm := TorusMesh.new()
		tm.inner_radius = 0.9
		tm.outer_radius = 1.0
		tm.rings = 32
		tm.ring_segments = 8
		_ring.mesh = tm
		_ring.material_override = Cel.material(color, 2.0, color, false)
		add_child(_ring)
	_born = DB.chart_time_now()
	scale = Vector3(0.05, 0.05, 0.05)
	look_at_stand()


func look_at_stand() -> void:
	var flat := Vector3(stand.x, global_position.y, stand.z)
	if flat.distance_to(global_position) > 0.01:
		look_at(flat, Vector3.UP)


## Marked for the Downbeat volley: the core turns to the volley colour.
func set_marked(on: bool) -> void:
	_marked = on
	_mat.set_shader_parameter("emission_color", Palette.YELLOW if on else Palette.KIND.get(kind, Color.WHITE))


## World position of the weak point (what the bot and the beam aim at).
func weak_position() -> Vector3:
	return weak.global_position


func update_cue(chart_time: float) -> void:
	if _dying:
		return
	# Pop in over the first `cue/pop_ms` of exposure.
	var age := chart_time - _born
	var s := clampf(age / (float(Settings.get_value("cue", "pop_ms")) / 1000.0), 0.05, 1.0)
	scale = Vector3(s, s, s)
	var span := maxf(hit_time - spawn_time, 0.001)
	var u := clampf((chart_time - spawn_time) / span, 0.0, 1.0)
	if _ring != null:
		var r := lerpf(radius * float(Settings.get_value("cue", "ring_start")), radius * 1.05, u)
		_ring.scale = Vector3(r, r, r)
		var cam := get_viewport().get_camera_3d()
		if cam != null:
			_ring.look_at(cam.global_position, Vector3.UP)
			_ring.rotate_object_local(Vector3.RIGHT, PI * 0.5)
	# Telegraph: the core burns brighter as the window closes.
	var late := clampf((chart_time - hit_time) / 0.16, 0.0, 1.0)
	var tele := float(Settings.get_value("cue", "telegraph"))
	_mat.set_shader_parameter("emission_strength", (4.5 if _marked else 0.8) + (1.2 * u + 3.0 * late) * tele)
	if kind == "TRACK":
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
	weak.collision_layer = 0
	weak.monitorable = false
	weak.remove_from_group("targets")
	body.collision_layer = 0
	match mode:
		"hit":
			_mat.set_shader_parameter("emission_strength", 6.0)
		"expired":
			_mat.set_shader_parameter("emission_strength", 0.0)
			_muzzle()
		_:
			pass


## A ghost killed this enemy here: a white ring that blooms and fades.
func ghost_flash() -> void:
	var ring := MeshInstance3D.new()
	var tm := TorusMesh.new()
	tm.inner_radius = radius * 1.1
	tm.outer_radius = radius * 1.3
	tm.rings = 24
	tm.ring_segments = 6
	ring.mesh = tm
	ring.material_override = Cel.material(Color(1, 1, 1, 0.9), 4.0, Color.WHITE, false)
	weak.add_child(ring)
	ring.look_at(stand, Vector3.UP)
	var tw := create_tween()
	tw.tween_property(ring, "scale", Vector3(2.2, 2.2, 2.2), 0.25)
	tw.parallel().tween_property(ring, "transparency", 1.0, 0.25)
	tw.tween_callback(ring.queue_free)


## Boss parts: a line from the part back into the boss body.
func tether_to(point: Vector3) -> void:
	if _tether != null:
		_tether.queue_free()
	var im := ImmediateMesh.new()
	im.surface_begin(Mesh.PRIMITIVE_LINES)
	im.surface_add_vertex(Vector3.ZERO)
	im.surface_add_vertex(to_local(point))
	im.surface_end()
	_tether = MeshInstance3D.new()
	_tether.mesh = im
	var color: Color = Palette.KIND.get(kind, Color.WHITE)
	_tether.material_override = Cel.material(Color(color, 0.5), 1.5, color, false)
	add_child(_tether)


## The enemy's shot: a tracer from the weak point toward the player stand.
func _muzzle() -> void:
	var cam := get_viewport().get_camera_3d()
	var to := cam.global_position if cam != null else stand
	var im := ImmediateMesh.new()
	im.surface_begin(Mesh.PRIMITIVE_LINES)
	im.surface_add_vertex(Vector3.ZERO)
	im.surface_add_vertex(to_local(to))
	im.surface_end()
	_flash = MeshInstance3D.new()
	_flash.mesh = im
	_flash.material_override = Cel.material(Color(1.0, 0.3, 0.2), 6.0, Color(1.0, 0.3, 0.2), false)
	add_child(_flash)


func _process(delta: float) -> void:
	if not _dying:
		return
	_die_t += delta
	match _die_mode:
		"hit":
			var s := 1.0 + _die_t * 6.0
			_weak_mesh.scale = Vector3(s, s, s)
			var b := maxf(1.0 - _die_t * 4.0, 0.0)
			_body_mesh.scale = Vector3(b, b, b)
			if _die_t > 0.14:
				queue_free()
		_:
			if _flash != null and _die_t > 0.09:
				_flash.visible = false
			var s := maxf(1.0 - (_die_t - 0.08) * 3.0, 0.0) if _die_t > 0.08 else 1.0
			scale = Vector3(s, s, s).max(Vector3(0.001, 0.001, 0.001))
			if _die_t > 0.45:
				queue_free()
