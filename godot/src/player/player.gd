extends CharacterBody3D
## First-person body: aim-trainer sensitivity (cm/360 at a DPI), timestamped
## fire, and shooter movement: walk, sprint, jump, slide, mantle, beat dash.
## Movement is free; the beat shows in its feel: head bob lands on the beat,
## the dash recharges on the bar and goes further when it starts on a beat.
## HP and damage come from the model through DB; this node only shows them.

@export var cm_per_360 := 34.0
@export var dpi := 800.0
@export var fov_vertical := 80.0
@export var walk_speed := 6.0
@export var sprint_speed := 9.5
@export var slide_speed := 12.5
@export var slide_time := 0.7
@export var accel := 60.0
@export var air_accel := 18.0
@export var jump_speed := 6.8
@export var gravity := 18.0
@export var mantle_height := 1.3
@export var dash_speed := 16.0
@export var dash_time := 0.16
## A dash started within this many ms of a beat is "on beat": longer and free.
@export var dash_window_ms := 90.0
@export var bob_amount := 0.035
@export var punch_fov := 3.0

@onready var head: Node3D = $Head
@onready var cam: Camera3D = $Head/Camera

var yaw := 0.0
var pitch := 0.0
var firing := false
var sprinting := false
var sliding := false
var dashing := false
var dash_ready := true
var marking := false
var _slide_t := 0.0
var _slide_dir := Vector3.ZERO
var _dash_t := 0.0
var _dash_dir := Vector3.ZERO
var _dash_len := 0.16
var _punch := 0.0
var _roll_dir := 1.0
var _shake := 0.0
var _moving := false
var _head_rest := 1.6
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
	_head_rest = head.position.y
	cam.keep_aspect = Camera3D.KEEP_HEIGHT
	cam.fov = fov_vertical
	cam.near = 0.05
	floor_snap_length = 0.3
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	Input.use_accumulated_input = false
	DB.run_started.connect(_on_run_started)
	DB.bar.connect(func(_i: int) -> void: dash_ready = true)
	DB.target_hit.connect(_on_hit)
	DB.volley.connect(func(_v: Dictionary) -> void: _punch = 1.6)
	DB.damaged.connect(func(_a: int, _hp: int, _t: int) -> void: _shake = 1.0)
	DB.beat.connect(_on_beat)
	DB.tick.connect(func(_i: int, _b: int, _s: int) -> void: DB.record_view(global_position, yaw, pitch))
	DB.cue.connect(func(c: Dictionary) -> void:
		if str(c.kind) == "shake":
			_shake = maxf(_shake, 1.0 + float(c.param)))
	DB.phase_end.connect(func(_p: int, _k: int, _t: int, cleared: bool, _l: bool) -> void:
		if not cleared:
			_shake = maxf(_shake, 1.6))


func _on_run_started(_bpm: float) -> void:
	# Start on the pad at the level origin, facing down the path.
	global_position = Vector3(0, 0.05, 0)
	velocity = Vector3.ZERO
	yaw = 0.0
	pitch = 0.0
	rotation.y = 0.0
	head.rotation.x = 0.0
	sliding = false


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


## Aim set from outside (replay watching).
func set_view(new_yaw: float, new_pitch: float) -> void:
	yaw = new_yaw
	pitch = clampf(new_pitch, -PI * 0.49, PI * 0.49)
	rotation.y = yaw
	head.rotation.x = pitch


func _unhandled_input(event: InputEvent) -> void:
	if DB.previewing or DB.analysing:
		return
	if DB.watching:
		# Watching a replay: the recording aims and shoots. R plays the level
		# yourself; Esc is the menu's.
		if event.is_action_pressed("restart"):
			DB.restart()
		return
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
	elif event.is_action_pressed("volley"):
		marking = DB.game != null and DB.game.volley_ready()
	elif event.is_action_released("volley"):
		if marking:
			marking = false
			DB.release(DB.input_song_time())
	elif event.is_action_pressed("dash"):
		_dash()
	elif event.is_action_pressed("restart"):
		DB.restart()
	elif event.is_action_pressed("calibrate"):
		DB.start_calibration()


func _on_hit(_id: int, v: Dictionary) -> void:
	# Camera punch sized by the verdict; roll alternates so a run of Perfects rocks.
	_punch = maxf(_punch, {"PERFECT": 1.0, "GREAT": 0.7, "GOOD": 0.45}.get(v.timing, 0.3))
	_roll_dir = -_roll_dir


func _on_beat(_index: int, _in_bar: int) -> void:
	if _moving and is_on_floor():
		DB.play_kit("STEP", -16.0 if not sprinting else -10.0)


## Beat dash: one per bar. Started inside the beat window it runs 40% longer
## and does not spend the charge.
func _dash() -> void:
	if dashing or sliding or not dash_ready or DB.game == null:
		return
	var input := Input.get_vector("move_left", "move_right", "move_forward", "move_back")
	var dir := (transform.basis * Vector3(input.x, 0, input.y)).normalized()
	if dir.length() < 0.1:
		dir = -transform.basis.z
	var off_ms := absf(DB.game.ms_from_nearest_beat(DB.input_song_time()))
	var on_beat := off_ms <= dash_window_ms
	dashing = true
	_dash_t = 0.0
	_dash_dir = dir
	_dash_len = dash_time * (1.4 if on_beat else 1.0)
	dash_ready = on_beat
	_punch = maxf(_punch, 0.6)
	DB.play_kit("DASH", -4.0 if on_beat else -12.0)
	DB.dashed.emit(on_beat)


func _fire(raw_song_time: float) -> void:
	var hit := aim_hit()
	if hit.is_empty():
		DB.fire(raw_song_time, -1, 0.0)
	elif hit.get("body", false):
		DB.chip(raw_song_time)
	else:
		DB.fire(raw_song_time, hit.id, hit.offset)


## Raycast from the camera. {} for nothing judgeable (walls, air),
## {body = true} for an enemy body (chip), or {id, offset, node, point} for a
## weak point. offset = perpendicular distance from the ray to the weak-point
## centre, so a rim clip and a centre punch are told apart.
func aim_hit() -> Dictionary:
	if _space == null:
		_space = get_world_3d().direct_space_state
	var from := cam.global_position
	var dir := -cam.global_transform.basis.z
	var q := PhysicsRayQueryParameters3D.create(from, from + dir * _ray_len)
	q.collide_with_areas = true
	q.collide_with_bodies = true
	var ex: Array[RID] = [get_rid()]
	q.exclude = ex
	var r := _space.intersect_ray(q)
	if r.is_empty():
		return {}
	var node: Node = r.collider
	if node == null:
		return {}
	if node.is_in_group("enemy_body"):
		return {"body": true, "point": r.position}
	if not node.is_in_group("targets"):
		return {}
	var centre: Vector3 = (node as Node3D).global_position
	var offset := (centre - from).cross(dir).length()
	return {"id": int(node.get_meta("target_id")), "offset": offset, "node": node, "point": r.position}


## Is the crosshair on this enemy's weak point right now (tracking)?
func is_on(enemy: Node) -> bool:
	var h := aim_hit()
	return not h.is_empty() and not h.get("body", false) and h.node == enemy.weak


func _physics_process(delta: float) -> void:
	if DB.watching:
		_camera(delta)
		return
	var input := Input.get_vector("move_left", "move_right", "move_forward", "move_back")
	var wish_dir := (transform.basis * Vector3(input.x, 0, input.y)).normalized()
	sprinting = Input.is_action_pressed("sprint") and input.y < 0.0 and is_on_floor()
	# Slide: sprinting + crouch on the floor; a decaying burst along the sprint direction.
	if Input.is_action_just_pressed("crouch") and sprinting and not sliding:
		sliding = true
		_slide_t = 0.0
		_slide_dir = wish_dir if wish_dir.length() > 0.1 else -transform.basis.z
	_moving = wish_dir.length() > 0.1 and is_on_floor()
	if dashing:
		_dash_t += delta
		var k := 1.0 - _dash_t / _dash_len
		velocity.x = _dash_dir.x * dash_speed * (0.4 + 0.6 * k)
		velocity.z = _dash_dir.z * dash_speed * (0.4 + 0.6 * k)
		velocity.y = maxf(velocity.y, 0.0)
		if _dash_t >= _dash_len:
			dashing = false
	elif sliding:
		_slide_t += delta
		var k := 1.0 - _slide_t / slide_time
		var speed := lerpf(walk_speed, slide_speed, k)
		velocity.x = _slide_dir.x * speed
		velocity.z = _slide_dir.z * speed
		if _slide_t >= slide_time or not is_on_floor() or Input.is_action_just_released("crouch"):
			sliding = false
	else:
		var target_speed := sprint_speed if sprinting else walk_speed
		var wish := wish_dir * target_speed
		var a := accel if is_on_floor() else air_accel
		velocity.x = move_toward(velocity.x, wish.x, a * delta)
		velocity.z = move_toward(velocity.z, wish.z, a * delta)
	if not is_on_floor():
		velocity.y -= gravity * delta
	elif Input.is_action_just_pressed("jump"):
		velocity.y = jump_speed
		sliding = false
	_mantle(wish_dir)
	move_and_slide()
	_camera(delta)
	if marking:
		var h := aim_hit()
		if not h.is_empty() and not h.get("body", false):
			DB.mark(int(h.id))


## Head bob on the beat phase while moving, punch and roll on verdicts,
## shake on damage. Nothing here touches the clock.
func _camera(delta: float) -> void:
	_punch = maxf(_punch - delta * 7.0, 0.0)
	_shake = maxf(_shake - delta * 4.0, 0.0)
	var phase := DB.beat_phase()
	var speed_k := clampf(Vector2(velocity.x, velocity.z).length() / sprint_speed, 0.0, 1.0)
	var bob := 0.0
	if _moving and not sliding:
		# Lands (lowest point) on the beat, rises through it.
		bob = -cos(phase * TAU) * bob_amount * speed_k
	var target_y := (_head_rest - 0.55 if sliding else _head_rest) + bob
	head.position.y = lerpf(head.position.y, target_y, 14.0 * delta)
	var shake := Vector3((randf() - 0.5) * 0.02, (randf() - 0.5) * 0.02, 0.0) * _shake
	cam.position = shake
	cam.fov = fov_vertical - punch_fov * _punch
	cam.rotation.z = deg_to_rad(0.8 * _punch * _roll_dir)


## Mantle: moving into a ledge lower than `mantle_height` with clear space
## above it lifts the body onto it instead of stopping dead.
func _mantle(wish_dir: Vector3) -> void:
	if wish_dir.length() < 0.1 or is_on_floor() and velocity.y <= 0.0 and not Input.is_action_pressed("jump"):
		return
	if _space == null:
		_space = get_world_3d().direct_space_state
	var base := global_position + Vector3(0, 0.4, 0)
	var ahead := wish_dir * 0.9
	var ex: Array[RID] = [get_rid()]
	var low := PhysicsRayQueryParameters3D.create(base, base + ahead)
	low.exclude = ex
	var high := PhysicsRayQueryParameters3D.create(base + Vector3(0, mantle_height, 0), base + Vector3(0, mantle_height, 0) + ahead)
	high.exclude = ex
	if not _space.intersect_ray(low).is_empty() and _space.intersect_ray(high).is_empty():
		velocity.y = maxf(velocity.y, 4.5)
