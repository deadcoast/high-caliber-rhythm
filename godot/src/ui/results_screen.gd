extends CanvasLayer
## Results: grade, score, pp, mods, FC; judgement histogram; timing histogram
## and scatter; per-kind table; this chart's leaderboard with the run marked;
## integrity notice. Greybox drawing (ASSETS.register.md [HUD-06]); the data
## and the actions are final.
##   R retry   Enter watch replay   Tab Range   Esc close

var canvas: Control
var report: Dictionary = {}
var board: Array = []
var integrity: Dictionary = {}
var meta: Dictionary = {}
var visible_now := false
var _font: Font
var _when := ""
var world: Dictionary = {}
var world_board: Array = []
var world_note := ""
var daily_date := ""


func _ready() -> void:
	layer = 2
	canvas = Control.new()
	canvas.set_anchors_preset(Control.PRESET_FULL_RECT)
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	canvas.visible = false
	canvas.draw.connect(_draw_results)
	add_child(canvas)
	_font = ThemeDB.fallback_font
	DB.run_finished.connect(_on_finished)
	DB.run_started.connect(func(_bpm: float) -> void: _hide())
	Online.submitted.connect(func(r: Dictionary) -> void:
		world = r
		world_note = ""
		canvas.queue_redraw())
	Online.board_received.connect(func(_fp: String, _m: String, b: Dictionary) -> void:
		world_board = b.get("entries", [])
		canvas.queue_redraw())
	Online.failed.connect(func(what: String, msg: String) -> void:
		if what in ["submit", "register", "board"]:
			world_note = "%s: %s" % [what, msg]
			canvas.queue_redraw())


func _on_finished(r: Dictionary) -> void:
	if DB.calibrating or DB.game == null:
		return
	report = r
	world = {}
	world_board = []
	world_note = "offline" if not Online.configured() else ("Range runs stay local" if not DB.current_drill.is_empty() else "posting…")
	daily_date = DB.daily_date
	meta = DB.game.level_meta()
	integrity = DB.game.integrity()
	board = Scores.board(str(DB.game.level_name()), str(DB.game.chart_fingerprint()), str(r.get("mods", "NM")))
	_when = DB.last_replay_path
	visible_now = true
	canvas.visible = true
	canvas.queue_redraw()


func _hide() -> void:
	visible_now = false
	canvas.visible = false


func _unhandled_input(event: InputEvent) -> void:
	if not visible_now:
		return
	if event.is_action_pressed("ui_accept"):
		if not DB.last_replay_path.is_empty():
			DB.watch_replay(DB.last_replay_path)
		get_viewport().set_input_as_handled()
	elif event is InputEventKey and event.is_pressed() and not event.is_echo() and (event as InputEventKey).keycode == KEY_G:
		# Race the world's best: fetch its replay, arm the ghost, play again.
		if not world_board.is_empty():
			var top: Dictionary = world_board[0]
			var ghost := get_parent().get_node_or_null("Ghost")
			if ghost != null:
				Online.replay_received.connect(func(_id: int, _json: String, _meta: Dictionary) -> void: DB.restart(), Object.CONNECT_ONE_SHOT)
				Online.fetch_replay(int(top.id), {"name": str(top.name), "score": int(top.score)})
				world_note = "fetching %s's replay…" % str(top.name)
				canvas.queue_redraw()
		get_viewport().set_input_as_handled()
	elif event.is_action_pressed("release_mouse"):
		_hide()
		get_viewport().set_input_as_handled()
	elif event.is_action_pressed("menu") or event.is_action_pressed("any_song"):
		# The Range or the song picker takes the screen; not consumed.
		_hide()


func _bar(x: float, y: float, w: float, h: float, col: Color) -> void:
	canvas.draw_rect(Rect2(x, y - h, w, h), col)


func _text(x: float, y: float, s: String, size: int = 16, col: Color = Color.WHITE) -> void:
	canvas.draw_string(_font, Vector2(x, y), s, HORIZONTAL_ALIGNMENT_LEFT, -1, size, col)


func _draw_results() -> void:
	var size := canvas.size
	canvas.draw_rect(Rect2(Vector2.ZERO, size), Color(Palette.INK, 0.88))
	var r := report
	var x0 := size.x * 0.5 - 560.0
	var y := 70.0
	# Header
	var title := "%s   %s   %s" % [str(meta.get("song", "")), str(meta.get("difficulty", "")), str(DB.game.level_name())]
	_text(x0, y, title.strip_edges(), 22, Color(1, 1, 1, 0.8))
	_text(x0 + 760, y, "%.0f BPM   %.2f stars   %s" % [DB.bpm, float(r.stars), str(r.mods)], 18, Color(1, 1, 1, 0.7))
	y += 70
	var grade_col: Color = Palette.CYAN if str(r.grade) in ["S", "A"] else (Palette.YELLOW if str(r.grade) in ["B", "C"] else Palette.RED)
	_text(x0, y, str(r.grade), 96, grade_col)
	_text(x0 + 110, y - 40, "SCORE %d" % int(r.score), 34)
	var flag := "PFC" if bool(r.pfc) else ("FC" if bool(r.fc) else "")
	_text(x0 + 110, y - 6, "%.0f pp     combo %d     %s" % [float(r.pp), int(r.max_combo), flag], 22, Palette.YELLOW)
	var boss_line := ""
	if int(r.get("phases_cleared", 0)) + int(r.get("phase_fails", 0)) > 0:
		boss_line = "   boss phases %d cleared, %d lost" % [int(r.phases_cleared), int(r.phase_fails)]
	_text(x0 + 110, y + 22, "hits %d/%d   aim %.1f%%   precision %.0f%%   on-beat %.1f%%   offset %+.1f ms   sd %.1f ms   damage %d   downs %d%s" % [
		int(r.hits), int(r.shots), float(r.aim_accuracy) * 100.0, float(r.precision) * 100.0, float(r.beat_accuracy) * 100.0,
		float(r.timing_mean_ms), float(r.timing_std_ms), int(r.damage_taken), int(r.downs), boss_line], 15, Color(1, 1, 1, 0.75))
	y += 70
	# Judgement histogram (left)
	var rows := [["PERFECT", int(r.perfect)], ["GREAT", int(r.great)], ["GOOD", int(r.good)], ["EARLY", int(r.early)], ["LATE", int(r.late)], ["MISS", int(r.expired)], ["WHIFF", int(r.whiffs)]]
	var maxc := 1
	for row in rows:
		maxc = maxi(maxc, int(row[1]))
	var jy := y
	for row in rows:
		var col: Color = Palette.JUDGE.get(str(row[0]), Color.WHITE)
		_text(x0, jy + 12, str(row[0]), 14, col)
		var w := 220.0 * float(row[1]) / float(maxc)
		canvas.draw_rect(Rect2(x0 + 80, jy, w, 12), col)
		_text(x0 + 90 + w, jy + 12, str(row[1]), 14, Color(1, 1, 1, 0.8))
		jy += 22
	# Timing histogram (middle): 10 ms bins, -110..110
	var hx := x0 + 360
	var hw := 330.0
	var hh := 120.0
	var hist: Array = r.get("history", [])
	var bins: Array[int] = []
	bins.resize(22)
	bins.fill(0)
	for shot in hist:
		var d := clampf(float(shot.delta_ms), -109.9, 109.9)
		bins[int((d + 110.0) / 10.0)] += 1
	var bmax := 1
	for b in bins:
		bmax = maxi(bmax, b)
	canvas.draw_rect(Rect2(hx, y, hw, hh), Color(0.08, 0.07, 0.12, 0.9))
	for i in bins.size():
		var cx := hx + i * hw / 22.0
		var ms := -110.0 + i * 10.0 + 5.0
		var col: Color = Palette.JUDGE.PERFECT if absf(ms) <= 30.0 else (Palette.JUDGE.GREAT if absf(ms) <= 60.0 else (Palette.JUDGE.GOOD if absf(ms) <= 100.0 else Palette.JUDGE.MISS))
		var bh := hh * bins[i] / float(bmax)
		canvas.draw_rect(Rect2(cx + 1, y + hh - bh, hw / 22.0 - 2, bh), Color(col, 0.85))
	canvas.draw_line(Vector2(hx + hw * 0.5, y), Vector2(hx + hw * 0.5, y + hh), Color(1, 1, 1, 0.5), 1.0)
	_text(hx, y + hh + 16, "timing distribution   early  <  0  >  late   10 ms bins", 13, Color(1, 1, 1, 0.55))
	# Scatter (right)
	var sx := hx + hw + 40
	var sw := 340.0
	canvas.draw_rect(Rect2(sx, y, sw, hh), Color(0.08, 0.07, 0.12, 0.9))
	if not hist.is_empty():
		var first: float = float(hist[0].hit_beat)
		var last: float = maxf(float(hist[hist.size() - 1].hit_beat), first + 1.0)
		var sy := hh * 0.5 / 110.0
		canvas.draw_rect(Rect2(sx, y + hh * 0.5 - 30.0 * sy, sw, 60.0 * sy), Color(Palette.CYAN, 0.10))
		canvas.draw_line(Vector2(sx, y + hh * 0.5), Vector2(sx + sw, y + hh * 0.5), Color(1, 1, 1, 0.4), 1.0)
		for shot in hist:
			var px := sx + (float(shot.hit_beat) - first) / (last - first) * sw
			var py := y + hh * 0.5 - clampf(float(shot.delta_ms), -110.0, 110.0) * sy
			canvas.draw_circle(Vector2(px, py), 2.5, Palette.JUDGE.get(str(shot.timing), Color.WHITE))
	_text(sx, y + hh + 16, "timing by beat   +-110 ms", 13, Color(1, 1, 1, 0.55))
	y += hh + 50
	# Per-kind table
	_text(x0, y, "%-10s %6s %6s %8s %8s %10s %7s" % ["KIND", "tgts", "hits", "on-beat", "expired", "precision", "track"], 14, Color(1, 1, 1, 0.6))
	y += 20
	var by: Dictionary = r.get("by_kind", {})
	for k in by:
		var s: Dictionary = by[k]
		var prec := ("%9.0f%%" % (float(s.precision) * 100.0)) if int(s.hits) > 0 and int(s.track_total) == 0 else "%10s" % "-"
		var track := ("%6.0f%%" % (100.0 * float(s.track_on) / float(maxi(int(s.track_total), 1)))) if int(s.track_total) > 0 else "%7s" % "-"
		_text(x0, y, "%-10s %6d %6d %8d %8d %s %s" % [str(k), int(s.targets), int(s.hits), int(s.on_beat), int(s.expired), prec, track], 14, Palette.KIND.get(str(k), Color.WHITE))
		y += 20
	# Leaderboard
	var lx := x0 + 620
	var ly := y - 20 * (by.size() + 1)
	_text(lx, ly, "LEADERBOARD   %s" % str(r.mods), 14, Color(1, 1, 1, 0.6))
	ly += 20
	for i in mini(board.size(), 5):
		var e: Dictionary = board[i]
		var mine := str(e.get("replay", "")) == _when and not _when.is_empty()
		var col := Palette.YELLOW if mine else Color.WHITE
		var line := "%d  %8d  %5.0f pp  %s  x%d  %s%s" % [i + 1, int(e.score), float(e.pp), str(e.grade), int(e.max_combo), str(e.get("when", "")).substr(0, 10), "  *" if mine else ""]
		if not bool(e.get("clean", true)):
			line += "  flagged"
		_text(lx, ly, line, 14, col)
		ly += 20
	if not bool(integrity.get("clean", true)):
		_text(lx, ly + 10, "held for review: " + ", ".join(integrity.get("flags", [])), 13, Palette.RED)
	# World
	var wx := lx + 360
	var wy := y - 20 * (by.size() + 1)
	if not world.is_empty():
		var rank_text := ("WORLD #%d" % int(world.rank)) if world.rank != null else "WORLD   not ranked (run not complete)"
		if not bool(world.get("ranked", true)):
			rank_text += "   custom chart board"
		if not bool(world.get("clean", true)):
			rank_text += "   held for review"
		if not daily_date.is_empty():
			rank_text = "DAILY %s   %s" % [daily_date, rank_text]
		_text(wx, wy, rank_text, 14, Palette.YELLOW)
	elif not world_note.is_empty():
		_text(wx, wy, world_note, 14, Color(1, 1, 1, 0.5))
	wy += 20
	for i in mini(world_board.size(), 5):
		var e: Dictionary = world_board[i]
		var mine := str(e.get("player", "")) == Online.player_id
		var line := "%d  %-12s %8d  %5.0f pp  %s%s" % [int(e.rank), str(e.name).left(12), int(e.score), float(e.pp), str(e.grade), "  *" if mine else ""]
		if not bool(e.get("clean", true)):
			line += "  flagged"
		_text(wx, wy, line, 14, Palette.YELLOW if mine else Color.WHITE)
		wy += 20
	if not world_board.is_empty():
		_text(wx, wy + 6, "[G] race #1 as a ghost", 13, Color(1, 1, 1, 0.55))
	# Actions
	_text(x0, size.y - 40, "[R] retry      [Enter] watch replay      [G] ghost race      [Tab] Range      [Esc] close", 15, Color(1, 1, 1, 0.6))
