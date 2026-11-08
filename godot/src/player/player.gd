extends CharacterBody3D
## First-person controller with aim-trainer sensitivity (cm/360 at a DPI) and
## timestamped fire. Movement is deliberately plain: the lane is where the game
## happens, the body is there so the arena reads as a place.

@export var cm_per_360 := 34.0
@export var dpi := 800.0
@export var fov_vertical := 80.0
@export var move_speed := 7.0
@export var accel := 60.0
@export var jump_speed := 6.5

@onready var head: Node3D = $Head
@onready var cam: Camera3D = $Head/Camera

var yaw := 0.0
var pitch := 0.0
var firing := false

var _space: PhysicsDirectSpaceState3D
var _ray_len := 200.0


func _ready() -> void:
	cm_per_360 = float(Settings.get_value("input", "cm_per_360"))
	dpi = float(Settings.get_value("input", "dpi"))
	fov_vertical = float(Settings.get_value("video", "fov"))
	Settings.changed.connect(_on_setting)
	var shape := CollisionShape3D.new()
	var capsule := CapsuleShape3D.new()
	capsule.radius = 0.35
	capsule.height = 1.8
	shape.shape = capsule
	shape.position = Vector3(0, 0.9, 0)
	add_child(shape)
	cam.keep_aspect = Camera3D.KEEP_HEIGHT
	cam.fov = fov_vertical
	cam.near = 0.05
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	Input.use_accumulated_input = false


func _on_setting(section: String, key: String, value: Variant) -> void:
	match [section, key]:
		["input", "cm_per_360"]:
			cm_per_360 = float(value)
		["input", "dpi"]:
			dpi = float(value)
		["video", "fov"]:
			fov_vertical = float(value)
			cam.fov = fov_vertical


func rad_per_count() -> float:
	var counts_per_360 := cm_per_360 / 2.54 * dpi
	return TAU / counts_per_360


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		var motion := event as InputEventMouseMotion
		var k := rad_per_count()
		yaw -= motion.relative.x * k
		pitch = clampf(pitch - motion.relative.y * k, -PI * 0.49, PI * 0.49)
		rotation.y = yaw
		head.rotation.x = pitch
	elif event.is_action_pressed("fire"):
		# Timestamp first, before any scene work, so pipeline time does not
		# leak into the judged click time.
		var raw := DB.input_song_time()
		firing = true
		_fire(raw)
	elif event.is_action_released("fire"):
		firing = false
	elif event.is_action_pressed("release_mouse"):
		Input.mouse_mode = Input.MOUSE_MODE_VISIBLE if Input.mouse_mode == Input.MOUSE_MODE_CAPTURED else Input.MOUSE_MODE_CAPTURED
	elif event.is_action_pressed("restart"):
		DB.restart()
	elif event.is_action_pressed("calibrate"):
		DB.start_calibration()


func _fire(raw_song_time: float) -> void:
	var hit := aim_hit()
	if hit.is_empty():
		DB.fire(raw_song_time, -1, 0.0)
	else:
		DB.fire(raw_song_time, hit.id, hit.offset)


## Raycast from the camera. Returns {} or {id, offset, node, point}.
## offset = perpendicular distance from the ray to the target centre, so it is
## the same whether the ray clipped the rim or hit the middle.
func aim_hit() -> Dictionary:
	if _space == null:
		_space = get_world_3d().direct_space_state
	var from := cam.global_position
	var dir := -cam.global_transform.basis.z
	var q := PhysicsRayQueryParameters3D.create(from, from + dir * _ray_len)
	q.collide_with_areas = true
	q.collide_with_bodies = true
	var r := _space.intersect_ray(q)
	if r.is_empty():
		return {}
	var node: Node = r.collider
	if node == null or not node.is_in_group("targets"):
		return {}
	var centre: Vector3 = node.global_position
	var offset := (centre - from).cross(dir).length()
	return {"id": node.target_id, "offset": offset, "node": node, "point": r.position}


## Is the crosshair on this specific target right now (tracking)?
func is_on(node: Node3D) -> bool:
	var h := aim_hit()
	return not h.is_empty() and h.node == node


func _physics_process(delta: float) -> void:
	var input := Input.get_vector("move_left", "move_right", "move_forward", "move_back")
	var wish := (transform.basis * Vector3(input.x, 0, input.y)).normalized() * move_speed
	velocity.x = move_toward(velocity.x, wish.x, accel * delta)
	velocity.z = move_toward(velocity.z, wish.z, accel * delta)
	if not is_on_floor():
		velocity.y -= 18.0 * delta
	elif Input.is_action_just_pressed("jump"):
		velocity.y = jump_speed
	move_and_slide()
