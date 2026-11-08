extends Node3D
## Greybox arena: a lane 12 m deep with a backdrop, side walls and a few
## blocks, lit for the toon ramp. Every mesh here is a placeholder shape
## (ASSETS.register.md [ENV-*]); the layout numbers are the real ones.

const FLOOR := Color(0.16, 0.17, 0.22)
const WALL := Color(0.22, 0.20, 0.30)
const BLOCK := Color(0.30, 0.28, 0.40)
const NEON_PINK := Color(1.0, 0.18, 0.70)
const NEON_CYAN := Color(0.15, 0.95, 1.0)


func _ready() -> void:
	_environment()
	_box(Vector3(0, -0.5, -6), Vector3(60, 1, 60), FLOOR)
	_box(Vector3(0, 6, -20), Vector3(60, 14, 1), WALL)           # backdrop
	_box(Vector3(-24, 6, -6), Vector3(1, 14, 60), WALL)           # left
	_box(Vector3(24, 6, -6), Vector3(1, 14, 60), WALL)            # right
	_box(Vector3(0, 6, 14), Vector3(60, 14, 1), WALL)             # behind
	_box(Vector3(-9, 2.5, -14), Vector3(2, 5, 2), BLOCK)
	_box(Vector3(9, 2.5, -14), Vector3(2, 5, 2), BLOCK)
	_box(Vector3(-16, 1, -4), Vector3(4, 2, 4), BLOCK)
	_box(Vector3(16, 1, -4), Vector3(4, 2, 4), BLOCK)
	_strip(Vector3(0, 0.02, -19.4), Vector3(40, 0.04, 0.3), NEON_PINK)
	_strip(Vector3(-23.4, 3.0, -6), Vector3(0.3, 0.08, 40), NEON_CYAN)
	_strip(Vector3(23.4, 3.0, -6), Vector3(0.3, 0.08, 40), NEON_CYAN)
	_strip(Vector3(0, 0.02, 0), Vector3(1.2, 0.04, 1.2), NEON_CYAN)       # lane origin mark


func _environment() -> void:
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.05, 0.04, 0.09)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.35, 0.30, 0.50)
	env.ambient_light_energy = 0.6
	env.glow_enabled = true
	env.glow_intensity = 0.6
	env.glow_bloom = 0.15
	var we := WorldEnvironment.new()
	we.environment = env
	add_child(we)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-48, 32, 0)
	sun.light_energy = 1.4
	sun.light_color = Color(1.0, 0.92, 0.85)
	sun.shadow_enabled = true
	add_child(sun)
	var fill := OmniLight3D.new()
	fill.position = Vector3(0, 7, -10)
	fill.omni_range = 30
	fill.light_energy = 0.8
	fill.light_color = Color(0.5, 0.3, 1.0)
	add_child(fill)


func _box(at: Vector3, size: Vector3, color: Color) -> void:
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
	add_child(body)


func _strip(at: Vector3, size: Vector3, color: Color) -> void:
	var mesh := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = size
	mesh.mesh = bm
	mesh.position = at
	mesh.material_override = Cel.material(color, 3.0, color, false)
	add_child(mesh)
