extends CanvasLayer
## The Range: pick a drill, a BPM and a duration; see your numbers per cell
## and your rank in the band; run the daily routine. Greybox UI drawn with
## primitives (ASSETS.register.md [UI-01]); the data and the flow are final.
##   Tab        open / close        Up / Down    drill
##   Left/Right BPM -/+ 2           PgDn / PgUp  BPM -/+ 10
##   - / =      minutes -/+ 1       Enter        start     D   daily routine
##   1 2 3 4 5  DT HT HR HD NF      O   any song (pick a file)

var open := false
var drills: Array = []
var index := 0
var bpm := 128.0
var minutes := 2.0
var routine_queue: Array[String] = []
var canvas: Control
var _font: Font


func _ready() -> void:
	layer = 2
	canvas = Control.new()
	canvas.set_anchors_preset(Control.PRESET_FULL_RECT)
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	canvas.visible = false
	canvas.draw.connect(_draw_menu)
	add_child(canvas)
	_font = ThemeDB.fallback_font
	bpm = DB.bpm
	DB.run_finished.connect(_on_finished)
	if DB.game != null:
		drills = DB.game.drills()


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("menu"):
		toggle()
		get_viewport().set_input_as_handled()
		return
	if not open:
		return
	if event.is_action_pressed("ui_down"):
		index = (index + 1) % maxi(drills.size(), 1)
	elif event.is_action_pressed("ui_up"):
		index = (index - 1 + maxi(drills.size(), 1)) % maxi(drills.size(), 1)
	elif event.is_action_pressed("ui_right"):
		bpm = minf(bpm + 2.0, 220.0)
	elif event.is_action_pressed("ui_left"):
		bpm = maxf(bpm - 2.0, 60.0)
	elif event.is_action_pressed("ui_page_up"):
		bpm = minf(bpm + 10.0, 220.0)
	elif event.is_action_pressed("ui_page_down"):
		bpm = maxf(bpm - 10.0, 60.0)
	elif event.is_action_pressed("minutes_up"):
		minutes = minf(minutes + 1.0, 10.0)
	elif event.is_action_pressed("minutes_down"):
		minutes = maxf(minutes - 1.0, 1.0)
	elif event.is_action_pressed("ui_accept"):
		_start(str(drills[index].name))
	elif event.is_action_pressed("routine"):
		routine_queue = Stats.routine(str(DB.game.bpm_band(bpm)))
		_next_in_routine()
	elif event.is_action_pressed("any_song"):
		toggle()
		var picker := get_parent().get_node_or_null("AnySong")
		if picker != null:
			picker.open()
	elif event is InputEventKey and event.is_pressed() and not event.is_echo():
		var k := (event as InputEventKey).keycode
		var mod := {KEY_1: "DT", KEY_2: "HT", KEY_3: "HR", KEY_4: "HD", KEY_5: "NF"}.get(k, "")
		if mod.is_empty():
			return
		DB.toggle_mod(mod)
	else:
		return
	get_viewport().set_input_as_handled()
	canvas.queue_redraw()


func toggle() -> void:
	open = not open
	canvas.visible = open
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE if open else Input.MOUSE_MODE_CAPTURED
	get_tree().paused = open
	canvas.queue_redraw()


func _start(drill: String) -> void:
	if open:
		toggle()
	DB.start_drill(drill, bpm, minutes)


func _next_in_routine() -> void:
	if routine_queue.is_empty():
		return
	var d: String = routine_queue.pop_front()
	_start(d)


func _on_finished(_report: Dictionary) -> void:
	if not routine_queue.is_empty():
		# Give the results a bar to breathe, then the next drill.
		get_tree().create_timer(3.0).timeout.connect(_next_in_routine)


func _draw_menu() -> void:
	var size := canvas.size
	canvas.draw_rect(Rect2(Vector2.ZERO, size), Color(0.03, 0.02, 0.06, 0.82))
	var x := size.x * 0.5 - 420.0
	var y := 90.0
	var band: String = str(DB.game.bpm_band(bpm)) if DB.game != null else ""
	canvas.draw_string(_font, Vector2(x, y), "RANGE", HORIZONTAL_ALIGNMENT_LEFT, -1, 40, Color(0.15, 0.95, 1.0))
	canvas.draw_string(_font, Vector2(x + 180, y), "%.0f BPM   band %s   rank %s   %.0f min" % [bpm, band, Stats.rank(band), minutes], HORIZONTAL_ALIGNMENT_LEFT, -1, 20, Color.WHITE)
	y += 28
	DB.game.set_mods(DB.mods)
	canvas.draw_string(_font, Vector2(x, y), "RATING %.0f pp     mods %s  x%.2f     [1] DT  [2] HT  [3] HR  [4] HD  [5] NF" % [Scores.profile_rating, DB.mods, float(DB.game.mod_multiplier())], HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Palette.YELLOW)
	y += 50
	canvas.draw_string(_font, Vector2(x, y), "%-12s %-7s %6s %8s %8s %8s %6s" % ["DRILL", "SKILL", "runs", "on-beat", "aim", "sd ms", "rank"], HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(1, 1, 1, 0.6))
	y += 28
	for i in drills.size():
		var d: Dictionary = drills[i]
		var s := Stats.summary(str(d.name), band)
		var col := Color(0.15, 0.95, 1.0) if i == index else Color.WHITE
		var line: String
		if int(s.runs) == 0:
			line = "%-12s %-7s %6s %8s %8s %8s %6s" % [str(d.name), str(d.skill), "-", "-", "-", "-", "-"]
		else:
			line = "%-12s %-7s %6d %7.0f%% %7.0f%% %8.1f %6s" % [str(d.name), str(d.skill), int(s.runs), float(s.beat_accuracy) * 100.0, float(s.aim_accuracy) * 100.0, float(s.timing_std_ms), str(DB.game.rank_for(float(s.composite)))]
		canvas.draw_string(_font, Vector2(x, y), ("> " if i == index else "  ") + line, HORIZONTAL_ALIGNMENT_LEFT, -1, 18, col)
		y += 28
	y += 16
	if index < drills.size():
		canvas.draw_string(_font, Vector2(x, y), str(drills[index].description), HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(1, 1, 1, 0.8))
	y += 40
	canvas.draw_string(_font, Vector2(x, y), "Up/Down drill   Left/Right BPM 2   PgUp/PgDn BPM 10   -/= minutes   Enter start   D daily routine   O any song   Tab close", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.5))
	y += 30
	var routine := Stats.routine(band)
	canvas.draw_string(_font, Vector2(x, y), "today: " + ", ".join(routine), HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.5))
