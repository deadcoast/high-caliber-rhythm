extends Node3D
## Viewmodel hooks: idle sway on the beat phase, recoil kick sized by the
## timing verdict, hold shake while tracking, settle after. The mesh is a
## placeholder block (ASSETS.register.md [WPN-01]); the motion curves are the
## backend the real model animates on top of.

const REST := Vector3(0.28, -0.22, -0.55)
const KICK := {"PERFECT": 1.0, "GREAT": 0.75, "GOOD": 0.5, "EARLY": 0.35, "LATE": 0.35}

var _kick := 0.0
var _kick_dir := 1.0
var _mesh: MeshInstance3D
var _mat: ShaderMaterial
var _flash := 0.0
var player: Node = null


func _ready() -> void:
	player = get_parent().get_parent().get_parent()
	position = REST
	_mesh = MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = Vector3(0.08, 0.12, 0.42)
	_mesh.mesh = bm
	_mesh.position = Vector3(0, 0, -0.1)
	_mat = Cel.material(Color(0.20, 0.19, 0.26), 0.0, Color(1.0, 0.18, 0.70))
	_mesh.material_override = _mat
	_mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(_mesh)
	var barrel := MeshInstance3D.new()
	var cyl := CylinderMesh.new()
	cyl.top_radius = 0.022
	cyl.bottom_radius = 0.022
	cyl.height = 0.18
	barrel.mesh = cyl
	barrel.rotation_degrees.x = 90.0
	barrel.position = Vector3(0, 0.02, -0.38)
	barrel.material_override = Cel.material(Color(0.12, 0.11, 0.16))
	barrel.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(barrel)
	DB.target_hit.connect(_on_hit)
	DB.whiff.connect(func(_v: Dictionary) -> void: _kick = 0.6)


func _on_hit(_id: int, v: Dictionary) -> void:
	_kick = float(KICK.get(v.timing, 0.5))
	_kick_dir = 1.0 if _kick_dir < 0.0 else -1.0
	_flash = 1.0 if v.on_beat else 0.2


func _process(delta: float) -> void:
	_kick = maxf(_kick - delta * 9.0, 0.0)
	_flash = maxf(_flash - delta * 10.0, 0.0)
	var phase := DB.beat_phase()
	# idle: a breath that lands on the beat
	var sway := Vector3(sin(phase * TAU) * 0.004, (1.0 - phase) * 0.006, 0.0)
	# recoil: back and up, settling with the kick
	var kick := Vector3(_kick_dir * _kick * 0.01, _kick * 0.03, _kick * 0.08)
	var shake := Vector3.ZERO
	if player != null and player.firing:
		shake = Vector3(sin(Time.get_ticks_msec() * 0.07) * 0.002, cos(Time.get_ticks_msec() * 0.09) * 0.002, 0.0)
	position = REST + sway + kick + shake
	rotation.x = _kick * 0.25
	_mat.set_shader_parameter("emission_strength", _flash * 4.0)
