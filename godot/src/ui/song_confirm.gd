extends CanvasLayer
## Any Song, step two: the measured grid over the song. The click layers play
## on the detected beats so a wrong tempo or offset is heard at once.
##   Left/Right offset -/+ 10 ms   Shift: 1 ms      Up/Down BPM x2 / /2
##   [ / ]  BPM -/+ 0.5             Space  skip 8 bars ahead
##   Enter  build the chart and play                 Esc    cancel

var open := false
var path := ""
var song_name := ""
var analysis: Dictionary = {}
var bpm := 128.0
var offset := 0.0
var canvas: Control
var _font: Font


func _ready() -> void:
	layer = 2
	canvas = Control.new()
	canvas.set_anchors_preset(Control.PRESET_FULL_RECT)
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	canvas.visible = false
	canvas.draw.connect(_draw_confirm)
	add_child(canvas)
	_font = ThemeDB.fallback_font
	DB.song_analysed.connect(_on_analysed)


func _on_analysed(song_path: String, song_name_in: String, a: Dictionary) -> void:
	if DB.skip_confirm:
		return
	path = song_path
	song_name = song_name_in
	analysis = a
	bpm = float(a.get("bpm", 128.0))
	offset = float(a.get("offset_s", 0.0))
	open = true
	canvas.visible = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	DB.preview_grid(path, bpm, offset)
	canvas.queue_redraw()


func _unhandled_input(event: InputEvent) -> void:
	if not open:
		return
	var fine := Input.is_key_pressed(KEY_SHIFT)
	var step := 0.001 if fine else 0.010
	if event.is_action_pressed("ui_right"):
		offset += step
	elif event.is_action_pressed("ui_left"):
		offset -= step
	elif event.is_action_pressed("ui_up"):
		bpm *= 2.0
	elif event.is_action_pressed("ui_down"):
		bpm *= 0.5
	elif event is InputEventKey and event.is_pressed() and not event.is_echo() and (event as InputEventKey).keycode == KEY_BRACKETLEFT:
		bpm -= 0.5
	elif event is InputEventKey and event.is_pressed() and not event.is_echo() and (event as InputEventKey).keycode == KEY_BRACKETRIGHT:
		bpm += 0.5
	elif event.is_action_pressed("ui_select"):
		DB.preview_seek_bars(8)
		get_viewport().set_input_as_handled()
		return
	elif event.is_action_pressed("ui_accept"):
		_confirm()
		get_viewport().set_input_as_handled()
		return
	elif event.is_action_pressed("release_mouse"):
		_close()
		get_viewport().set_input_as_handled()
		return
	else:
		return
	bpm = clampf(bpm, 40.0, 300.0)
	var spb := 60.0 / bpm
	while offset < 0.0:
		offset += spb
	while offset >= spb:
		offset -= spb
	analysis = DB.analysis_game.regrid_file(path, bpm, offset)
	DB.preview_grid(path, bpm, offset)
	get_viewport().set_input_as_handled()
	canvas.queue_redraw()


func _confirm() -> void:
	_close()
	DB.build_any_song(path, song_name, bpm, offset, analysis)


func _close() -> void:
	open = false
	canvas.visible = false
	DB.stop_preview()
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _draw_confirm() -> void:
	var size := canvas.size
	canvas.draw_rect(Rect2(Vector2.ZERO, size), Color(Palette.INK, 0.9))
	var x := size.x * 0.5 - 480.0
	var y := 100.0
	canvas.draw_string(_font, Vector2(x, y), "ANY SONG   " + song_name, HORIZONTAL_ALIGNMENT_LEFT, -1, 32, Palette.CYAN)
	y += 50
	canvas.draw_string(_font, Vector2(x, y), "%.2f BPM     beat 0 at %.3f s     %d bars     tempo confidence %.1fx   phase %.1fx" % [
		bpm, offset, int(analysis.get("bars", 0)), float(analysis.get("tempo_confidence", 0.0)), float(analysis.get("phase_confidence", 0.0))], HORIZONTAL_ALIGNMENT_LEFT, -1, 18, Color.WHITE)
	y += 50
	# Onset grid: 64 sixteenth slots over four bars; bar lines every 16.
	var grid: Array = analysis.get("onset_grid", [])
	var w := 960.0
	var h := 120.0
	canvas.draw_rect(Rect2(x, y, w, h), Color(0.08, 0.07, 0.12, 0.9))
	for k in grid.size():
		var gx := x + k * w / 64.0
		var v := clampf(float(grid[k]), 0.0, 1.0)
		var col: Color = Palette.MAGENTA if k % 4 == 0 else Palette.CYAN
		canvas.draw_rect(Rect2(gx + 1, y + h - v * h, w / 64.0 - 2, v * h), Color(col, 0.85))
	for b in 5:
		var lx := x + b * w / 4.0
		canvas.draw_line(Vector2(lx, y), Vector2(lx, y + h), Color(1, 1, 1, 0.5), 1.0)
	canvas.draw_string(_font, Vector2(x, y + h + 18), "onsets on the grid, four bars from beat 0.  Beats light magenta; off-beats cyan. If the tall bars sit between the lines, nudge.", HORIZONTAL_ALIGNMENT_LEFT, -1, 13, Color(1, 1, 1, 0.6))
	y += h + 50
	# Energy per bar
	var energy: Array = analysis.get("energy", [])
	canvas.draw_string(_font, Vector2(x, y), "energy per bar (the chart's intensity follows it)", HORIZONTAL_ALIGNMENT_LEFT, -1, 13, Color(1, 1, 1, 0.6))
	y += 10
	var ew := minf(w / maxi(energy.size(), 1), 24.0)
	for i in energy.size():
		var v := clampf(float(energy[i]), 0.0, 1.0)
		canvas.draw_rect(Rect2(x + i * ew, y + 60 - v * 60, ew - 1, v * 60), Color(Palette.YELLOW, 0.8))
	y += 100
	canvas.draw_string(_font, Vector2(x, y), "Left/Right offset 10 ms (Shift 1 ms)   Up/Down BPM x2 / /2   [ ] BPM 0.5   Space skip 8 bars   Enter build and play   Esc cancel", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.5))
	y += 24
	canvas.draw_string(_font, Vector2(x, y), "The click you hear is the grid. When it sits on the kick, Enter.", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.5))
