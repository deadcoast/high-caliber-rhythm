extends CanvasLayer
## The menu: PLAY (level select), SETTINGS, and the pause screen. Greybox
## drawing (ASSETS.register.md [UI-01]); the data and the flow are final.
## Opens on launch when no run was asked for on the command line.
##   Esc   pause a run / close           Up Down   row       Enter  start / apply
##   P S   PLAY / SETTINGS               Left Right adjust (BPM of a builtin, a setting)
##   1-5   DT HT HR HD NF                Tab Range   O any song   C calibrate (settings)
##   Backspace  reset a setting (Settings: all)      Q  quit (pause)

const PLAY := 0
const SETTINGS_SCREEN := 1
const PAUSE := 2

const BUILTINS := [
	["street", "STREET", "Act one, level one: street, atrium, platform"],
	["street_hard", "STREET HARD", "The same path, every pattern denser"],
	["warmup", "WARMUP", "Range reference: everything once, in a musical order"],
	["fill", "FILL", "Range: every enemy kind including the heavy"],
]
## section, key, label, step, min, max, unit
const SETTINGS := [
	["input", "cm_per_360", "sensitivity", 0.5, 5.0, 120.0, "cm/360"],
	["input", "dpi", "mouse DPI", 50.0, 100.0, 16000.0, ""],
	["input", "offset_ms", "input offset (C calibrates)", 1.0, -150.0, 150.0, "ms"],
	["video", "fov", "field of view (vertical)", 1.0, 60.0, 120.0, "deg"],
	["video", "max_fps", "frame cap (0 = none)", 30.0, 0.0, 480.0, "fps"],
	["video", "postfx", "post-FX strength", 0.05, 0.0, 1.0, ""],
	["video", "aberration", "chromatic split", 0.05, 0.0, 1.0, ""],
	["video", "scanlines", "scanlines", 0.05, 0.0, 1.0, ""],
	["audio", "master_db", "master volume", 1.0, -40.0, 6.0, "dB"],
	["cue", "ring_start", "approach ring start size", 0.1, 1.5, 5.0, "x radius"],
	["cue", "pop_ms", "weak point pop-in", 10.0, 40.0, 300.0, "ms"],
	["cue", "telegraph", "enemy fire telegraph", 0.1, 0.0, 2.0, ""],
]
const PAUSE_ITEMS := ["RESUME", "RESTART", "LEVEL SELECT", "SETTINGS", "RANGE", "QUIT"]
## Text settings edited through a prompt (Enter on the row).
const TEXT_SETTINGS := [
	["online", "url", "server url (empty = offline)"],
	["online", "name", "player name"],
]

var open := false
var screen := PLAY
var row := 0
var builtin_bpm := 128.0
var canvas: Control
var _font: Font
var _rows: Array[Dictionary] = []
var _rows_key := ""
var _prompt: AcceptDialog
var _prompt_edit: LineEdit
var _daily_rank := ""


func _ready() -> void:
	layer = 4
	canvas = Control.new()
	canvas.set_anchors_preset(Control.PRESET_FULL_RECT)
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	canvas.visible = false
	canvas.draw.connect(_draw_menu)
	add_child(canvas)
	_font = ThemeDB.fallback_font
	builtin_bpm = DB.bpm
	DB.run_started.connect(func(_bpm: float) -> void:
		if open:
			close())
	_prompt = AcceptDialog.new()
	_prompt.title = "Setting"
	_prompt_edit = LineEdit.new()
	_prompt_edit.custom_minimum_size = Vector2(520, 0)
	_prompt.add_child(_prompt_edit)
	_prompt.register_text_enter(_prompt_edit)
	add_child(_prompt)
	Online.daily_received.connect(func(_d: Dictionary) -> void: canvas.queue_redraw())
	Online.board_received.connect(func(fp: String, _m: String, b: Dictionary) -> void:
		if fp == "daily":
			_daily_rank = ""
			for e in b.get("entries", []):
				if str(e.get("player", "")) == Online.player_id:
					_daily_rank = "#%d  %d" % [int(e.rank), int(e.score)]
			canvas.queue_redraw())


func _unhandled_input(event: InputEvent) -> void:
	var range_menu := get_parent().get_node_or_null("RangeMenu")
	var results := get_parent().get_node_or_null("Results")
	if not open:
		if event.is_action_pressed("release_mouse"):
			if range_menu != null and range_menu.open:
				range_menu.toggle()
			elif results != null and results.visible_now:
				return
			elif DB.previewing or DB.analysing:
				return
			elif DB.running:
				open_screen(PAUSE)
			else:
				open_screen(PLAY)
			get_viewport().set_input_as_handled()
		return
	if event.is_action_pressed("release_mouse"):
		if screen == PAUSE or not DB.running:
			close()
		else:
			open_screen(PAUSE)
	elif event.is_action_pressed("menu"):
		close()
		if range_menu != null:
			range_menu.toggle()
	elif event.is_action_pressed("any_song"):
		close()
		var picker := get_parent().get_node_or_null("AnySong")
		if picker != null:
			picker.open()
	elif event.is_action_pressed("ui_down"):
		row = (row + 1) % maxi(_row_count(), 1)
	elif event.is_action_pressed("ui_up"):
		row = (row - 1 + maxi(_row_count(), 1)) % maxi(_row_count(), 1)
	elif event.is_action_pressed("ui_right"):
		_adjust(1)
	elif event.is_action_pressed("ui_left"):
		_adjust(-1)
	elif event.is_action_pressed("ui_page_up"):
		_adjust(5)
	elif event.is_action_pressed("ui_page_down"):
		_adjust(-5)
	elif event.is_action_pressed("ui_accept"):
		_accept()
	elif event.is_action_pressed("calibrate") and screen == SETTINGS_SCREEN:
		close()
		DB.start_calibration()
	elif event is InputEventKey and event.is_pressed() and not event.is_echo():
		var k := (event as InputEventKey).keycode
		if k == KEY_P:
			open_screen(PLAY)
		elif k == KEY_S:
			open_screen(SETTINGS_SCREEN)
		elif k == KEY_Q and screen == PAUSE:
			get_tree().quit()
		elif k == KEY_BACKSPACE and screen == SETTINGS_SCREEN:
			var r: Array = SETTINGS[row] if row < SETTINGS.size() else TEXT_SETTINGS[row - SETTINGS.size()]
			Settings.set_value(str(r[0]), str(r[1]), Settings.DEFAULTS[r[0]][r[1]])
		elif k >= KEY_1 and k <= KEY_5 and screen == PLAY:
			DB.toggle_mod(["DT", "HT", "HR", "HD", "NF"][k - KEY_1])
			_rows_key = ""
		else:
			return
	else:
		return
	get_viewport().set_input_as_handled()
	canvas.queue_redraw()


func open_screen(which: int) -> void:
	screen = which
	row = 0
	open = true
	canvas.visible = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	get_tree().paused = true
	if which == PLAY and Online.configured() and Online.daily_fetched_on != Time.get_date_string_from_system(true):
		Online.fetch_daily()
		Online.fetch_daily_board()
	canvas.queue_redraw()


func close() -> void:
	open = false
	canvas.visible = false
	get_tree().paused = false
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _campaign_rows() -> Array:
	var out := []
	for a in Campaign.ACTS.size():
		var levels: Array = Campaign.ACTS[a].levels
		for i in levels.size():
			out.append({"act": a, "index": i, "level": levels[i], "unlocked": Campaign.is_unlocked(a, i)})
	return out


func _daily_rows() -> int:
	return 1 if Online.configured() else 0


func _row_count() -> int:
	match screen:
		PLAY:
			return _daily_rows() + _campaign_rows().size() + BUILTINS.size() + _level_rows().size()
		SETTINGS_SCREEN:
			return SETTINGS.size() + TEXT_SETTINGS.size()
		_:
			return PAUSE_ITEMS.size()


## File levels rated under the current mods; cached until mods or BPM change.
func _level_rows() -> Array[Dictionary]:
	var key := DB.mods
	if key != _rows_key:
		_rows = Levels.index(DB.mods)
		_rows_key = key
	return _rows


func _adjust(dir: int) -> void:
	match screen:
		PLAY:
			var nc := _daily_rows() + _campaign_rows().size()
			if row >= nc and row < nc + BUILTINS.size():
				builtin_bpm = clampf(builtin_bpm + 2.0 * dir, 60.0, 220.0)
		SETTINGS_SCREEN:
			if row >= SETTINGS.size():
				return
			var r: Array = SETTINGS[row]
			var v := float(Settings.get_value(str(r[0]), str(r[1]))) + float(r[3]) * dir
			v = clampf(v, float(r[4]), float(r[5]))
			v = snappedf(v, float(r[3]))
			Settings.set_value(str(r[0]), str(r[1]), v)
		_:
			pass


func _accept() -> void:
	match screen:
		PLAY:
			var nd := _daily_rows()
			if row < nd:
				if Online.daily.is_empty():
					Online.fetch_daily()
					return
				close()
				Online.start_daily()
				return
			var campaign := _campaign_rows()
			var nc := nd + campaign.size()
			if row < nc:
				var c: Dictionary = campaign[row - nd]
				if not bool(c.unlocked):
					return
				close()
				DB.start_level(str(c.level.path))
			elif row < nc + BUILTINS.size():
				close()
				DB.start_builtin(str(BUILTINS[row - nc][0]), builtin_bpm)
			else:
				var lv: Dictionary = _level_rows()[row - nc - BUILTINS.size()]
				close()
				DB.start_level(str(lv.path))
		SETTINGS_SCREEN:
			if row >= SETTINGS.size():
				var t: Array = TEXT_SETTINGS[row - SETTINGS.size()]
				_prompt.title = str(t[2])
				_prompt_edit.text = str(Settings.get_value(str(t[0]), str(t[1])))
				_prompt.confirmed.connect(func() -> void:
					Settings.set_value(str(t[0]), str(t[1]), _prompt_edit.text.strip_edges())
					canvas.queue_redraw(), Object.CONNECT_ONE_SHOT)
				_prompt.popup_centered()
				_prompt_edit.grab_focus()
		PAUSE:
			match PAUSE_ITEMS[row]:
				"RESUME":
					close()
				"RESTART":
					close()
					DB.restart()
				"LEVEL SELECT":
					open_screen(PLAY)
				"SETTINGS":
					open_screen(SETTINGS_SCREEN)
				"RANGE":
					close()
					var range_menu := get_parent().get_node_or_null("RangeMenu")
					if range_menu != null:
						range_menu.toggle()
				"QUIT":
					get_tree().quit()


func _text(x: float, y: float, s: String, size: int = 16, col: Color = Color.WHITE) -> void:
	canvas.draw_string(_font, Vector2(x, y), s, HORIZONTAL_ALIGNMENT_LEFT, -1, size, col)


func _draw_menu() -> void:
	var size := canvas.size
	canvas.draw_rect(Rect2(Vector2.ZERO, size), Color(Palette.INK, 0.9))
	var x := size.x * 0.5 - 560.0
	var y := 80.0
	_text(x, y, "DOWNBEAT", 40, Palette.YELLOW)
	var tabs := "[P] PLAY     [S] SETTINGS     [Tab] RANGE     [O] ANY SONG"
	_text(x + 260, y - 4, tabs, 16, Color(1, 1, 1, 0.6))
	_text(x + 260, y + 18, "rating %.0f pp" % Scores.profile_rating, 14, Palette.CYAN)
	y += 60
	match screen:
		PLAY:
			_draw_play(x, y)
		SETTINGS_SCREEN:
			_draw_settings(x, y)
		PAUSE:
			_draw_pause(x, y)


func _draw_play(x: float, y: float) -> void:
	if DB.game != null:
		DB.game.set_mods(DB.mods)
		_text(x, y, "mods %s  x%.2f     [1] DT  [2] HT  [3] HR  [4] HD  [5] NF" % [DB.mods, float(DB.game.mod_multiplier())], 15, Palette.YELLOW)
	y += 34
	var nd := _daily_rows()
	if nd > 0:
		var sel_d := row == 0
		var d := Online.daily
		var dt := "DAILY   " + (("%s   %s" % [str(d.get("title", "")), str(d.get("date", ""))]) if not d.is_empty() else "fetching…")
		if not _daily_rank.is_empty():
			dt += "   you " + _daily_rank
		_text(x, y, ("> " if sel_d else "  ") + dt, 16, Palette.CYAN if sel_d else Palette.YELLOW)
		y += 30
	var campaign := _campaign_rows()
	var nc := campaign.size()
	var cleared := Campaign.act_is_clear(0)
	_text(x, y, "%s%s" % [str(Campaign.ACTS[0].name), "   CLEAR" if cleared else ""], 16, Palette.YELLOW if cleared else Color(1, 1, 1, 0.7))
	y += 24
	for i in nc:
		var c: Dictionary = campaign[i]
		var sel := i + nd == row
		var lv: Dictionary = c.level
		var b := Campaign.best(str(lv.name))
		var state := ""
		if not bool(c.unlocked):
			state = "LOCKED   clear the level before it with a C"
		elif b.is_empty():
			state = "not played"
		else:
			state = "best %s   %d   %.0f pp   %d plays%s" % [str(b.grade), int(b.score), float(b.pp), int(b.plays), "   CLEARED" if bool(b.cleared) else ""]
		var col: Color = Palette.CYAN if sel else (Color(1, 1, 1, 0.35) if not bool(c.unlocked) else Color.WHITE)
		_text(x, y, ("> " if sel else "  ") + "%d.  %-16s %s" % [int(c.index) + 1, str(lv.title), state], 16, col)
		y += 24
	y += 10
	_text(x, y, "%-26s %-22s %-8s %6s %6s %8s %5s %6s" % ["LEVEL", "SONG", "DIFF", "BPM", "STARS", "BEST", "GRADE", "PP"], 14, Color(1, 1, 1, 0.6))
	y += 24
	var levels := _level_rows()
	var total := BUILTINS.size() + levels.size()
	for j in total:
		var i := j + nc + nd
		var sel := i == row
		var col := Palette.CYAN if sel else Color.WHITE
		var line: String
		var desc := ""
		if j < BUILTINS.size():
			var b: Array = BUILTINS[j]
			var info: Dictionary = {}
			if DB.game != null:
				info = DB.game.rate_level_json_mods(DB.game.builtin_level_json(str(b[0]), builtin_bpm), DB.mods)
			var best := Scores.best(str(info.get("name", "")), str(info.get("fingerprint", "")), DB.mods) if not info.is_empty() else {}
			line = "%-26s %-22s %-8s %6.0f %6.2f %8s %5s %6s" % [str(b[1]), "click kit", "-", builtin_bpm, float(info.get("stars", 0.0)),
				str(best.get("score", "-")), str(best.get("grade", "-")), ("%.0f" % float(best.pp)) if best.has("pp") else "-"]
			desc = str(b[2]) + "   (Left/Right BPM)"
		else:
			var lv: Dictionary = levels[j - BUILTINS.size()]
			var best: Dictionary = lv.best
			var tag := "song: " if str(lv.source) == "song" else ""
			var song := (str(lv.song) + ("  " + str(lv.artist) if not str(lv.artist).is_empty() else "")) if not str(lv.song).is_empty() else ("stems" if int(lv.stems) > 0 else ("click" if str(lv.audio).is_empty() else str(lv.audio).get_file()))
			line = "%-26s %-22s %-8s %6.0f %6.2f %8s %5s %6s" % [(tag + str(lv.name)).left(26), song.left(22), str(lv.difficulty).left(8), float(lv.bpm), float(lv.stars),
				str(best.get("score", "-")), str(best.get("grade", "-")), ("%.0f" % float(best.pp)) if best.has("pp") else "-"]
			desc = "%d bars   %d enemies   %.0f s   %s" % [int(lv.bars), int(lv.targets), float(lv.duration_s), "by " + str(lv.charter) if not str(lv.charter).is_empty() else ""]
		_text(x, y, ("> " if sel else "  ") + line, 16, col)
		if sel:
			_text(x + 640, y, desc, 13, Color(1, 1, 1, 0.55))
		y += 24
	y += 20
	_text(x, y, "Up/Down level   Enter play   1-5 mods   Tab Range   O any song   Esc " + ("resume" if DB.running else "close"), 14, Color(1, 1, 1, 0.5))


func _draw_settings(x: float, y: float) -> void:
	for i in SETTINGS.size():
		var r: Array = SETTINGS[i]
		var sel := i == row
		var v := float(Settings.get_value(str(r[0]), str(r[1])))
		var shown := ("%.2f" % v) if float(r[3]) < 1.0 else ("%.0f" % v)
		var line := "%-34s %10s %s" % [str(r[2]), shown, str(r[6])]
		_text(x, y, ("> " if sel else "  ") + line, 16, Palette.CYAN if sel else Color.WHITE)
		y += 26
	for i in TEXT_SETTINGS.size():
		var t: Array = TEXT_SETTINGS[i]
		var sel := i + SETTINGS.size() == row
		var v := str(Settings.get_value(str(t[0]), str(t[1])))
		_text(x, y, ("> " if sel else "  ") + "%-34s %s" % [str(t[2]), v if not v.is_empty() else "-"], 16, Palette.CYAN if sel else Color.WHITE)
		y += 26
	if Online.configured():
		_text(x, y, "online as %s%s" % [Online.player_name, ("   id " + Online.player_id.left(8)) if not Online.player_id.is_empty() else "   (registers on the first score)"], 14, Color(1, 1, 1, 0.55))
		y += 26
	y += 10
	var cm := float(Settings.get_value("input", "cm_per_360"))
	var dpi := float(Settings.get_value("input", "dpi"))
	_text(x, y, "%.0f counts per 360   %.4f deg per count" % [cm / 2.54 * dpi, 360.0 / maxf(cm / 2.54 * dpi, 1.0)], 14, Color(1, 1, 1, 0.55))
	y += 30
	_text(x, y, "Left/Right adjust   PgUp/PgDn x5   Backspace default   Enter edit a text row   C calibrate the offset   Esc back", 14, Color(1, 1, 1, 0.5))


func _draw_pause(x: float, y: float) -> void:
	_text(x, y, "PAUSED", 28, Palette.CYAN)
	y += 40
	for i in PAUSE_ITEMS.size():
		var sel := i == row
		_text(x, y, ("> " if sel else "  ") + PAUSE_ITEMS[i], 18, Palette.CYAN if sel else Color.WHITE)
		y += 28
	y += 10
	if DB.game != null and DB.running:
		_text(x, y, "%s   %.0f BPM   %s   score %d   combo %d   hp %d" % [str(DB.game.level_name()), DB.bpm, str(DB.game.mods()), int(DB.game.score()), int(DB.game.combo()), int(DB.game.hp())], 14, Color(1, 1, 1, 0.6))
