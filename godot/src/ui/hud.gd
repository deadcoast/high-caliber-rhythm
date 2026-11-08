extends CanvasLayer
## Greybox HUD. Crosshair ring collapses onto the dot on every beat, judgement
## pops under it, timing meter shows where the last shot landed in the window.
## The results page is its own screen (results_screen.gd).
## All typography, colour and layout are placeholders (ASSETS.register.md [HUD-*]).

const PERFECT_MS := 30.0
const GREAT_MS := 60.0
const GOOD_MS := 100.0

const JUDGE_COLORS := Palette.JUDGE

var canvas: Control
var _judge_text := ""
var _judge_color := Color.WHITE
var _judge_age := 9.0
var _last_delta := 0.0
var _has_delta := false
var _beat_flash := 0.0
var _font: Font
var _tier := 0
var _best: Dictionary = {}
var _stars := 0.0
var _hp := 100
var _hurt := 0.0
var _spans: Array = []
var _phases: Array = []
var _boss_start := 1e9
var _ghost: Node = null


func _ready() -> void:
	canvas = Control.new()
	canvas.set_anchors_preset(Control.PRESET_FULL_RECT)
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	canvas.draw.connect(_draw_hud)
	add_child(canvas)
	_font = ThemeDB.fallback_font
	DB.target_hit.connect(_on_hit)
	DB.whiff.connect(_on_whiff)
	DB.target_expired.connect(func(_id: int) -> void: _judge("MISS", JUDGE_COLORS.MISS))
	DB.track_done.connect(_on_track)
	DB.beat.connect(func(_i: int, _b: int) -> void: _beat_flash = 1.0)
	DB.run_started.connect(_on_started)
	DB.calibrated.connect(func(ms: float) -> void: _judge("OFFSET %+.0f ms" % ms, JUDGE_COLORS.PERFECT))
	DB.tier_changed.connect(func(t: int) -> void: _tier = t)
	DB.damaged.connect(_on_damaged)
	DB.down.connect(func(downs: int) -> void:
		_hp = 100
		_judge("DOWN %d" % downs, JUDGE_COLORS.MISS))
	DB.chipped.connect(func() -> void: _judge("CHIP", JUDGE_COLORS.WHIFF))
	DB.restarted.connect(func(_seg: int, _t: float) -> void:
		_hp = 100
		_judge("RETRY", JUDGE_COLORS.GOOD))
	DB.volley.connect(func(v: Dictionary) -> void: _judge("DOWNBEAT x%d" % (v.kills as Array).size(), Palette.YELLOW))
	DB.volley_miss.connect(func(_v: Dictionary) -> void: _judge("OFF THE BAR", JUDGE_COLORS.MISS))
	DB.dashed.connect(func(on_beat: bool) -> void:
		if on_beat:
			_judge("ON BEAT", JUDGE_COLORS.GREAT))
	DB.phase_end.connect(func(phase: int, kills: int, total: int, cleared: bool, last: bool) -> void:
		if cleared and last:
			_judge("BOSS DOWN", Palette.YELLOW)
		elif cleared:
			_judge("PHASE %d CLEAR  %d/%d" % [phase + 1, kills, total], JUDGE_COLORS.PERFECT)
		else:
			_judge("PHASE LOST  %d/%d" % [kills, total], JUDGE_COLORS.MISS))
	Scores.new_best.connect(func(_level: String, _r: Dictionary) -> void: _judge("NEW BEST", JUDGE_COLORS.PERFECT))
	_ghost = get_parent().get_node_or_null("Ghost")
	if _ghost != null:
		_ghost.ghost_restart.connect(func() -> void: _judge("GHOST DOWN", JUDGE_COLORS.WHIFF))


func _on_started(_bpm: float) -> void:
	_has_delta = false
	_hp = 100
	_hurt = 0.0
	if DB.game != null:
		_best = Scores.best(str(DB.game.level_name()), str(DB.game.chart_fingerprint()), str(DB.game.mods()))
		_stars = float(DB.game.rating().get("stars", 0.0))
		_spans = DB.game.spans()
		_phases = DB.game.phases()
		_boss_start = float(_phases[0].start_time) if not _phases.is_empty() else 1e9


func _on_damaged(amount: int, hp: int, _target: int) -> void:
	_hp = hp
	_hurt = 1.0
	_judge("-%d" % amount, JUDGE_COLORS.MISS)


func _phase_clear(phase: int) -> float:
	for p in _phases:
		if int(p.phase) == phase:
			return float(p.clear)
	return 0.7


## Current segment label and its remaining bars, for the top-centre strip.
func _segment_text() -> String:
	if _spans.is_empty() or DB.game == null:
		return ""
	var beat: float = DB.game.song_beat()
	for sp in _spans:
		if beat >= float(sp.start_beat) and beat < float(sp.end_beat):
			var bars_left := (float(sp.end_beat) - beat) / 4.0
			if sp.kind == "traversal":
				return "MOVE   %.1f bars" % bars_left
			if sp.kind == "boss":
				return ""
			return "%s   %.1f bars" % [str(sp.room).to_upper(), bars_left]
	return "LEAD-IN" if beat < float(_spans[0].start_beat) else "CLEAR"


func _on_hit(_id: int, v: Dictionary) -> void:
	_last_delta = v.delta_ms
	_has_delta = true
	_judge(v.timing, JUDGE_COLORS.get(v.timing, Color.WHITE))


func _on_whiff(v: Dictionary) -> void:
	_last_delta = v.delta_ms
	_has_delta = true
	_judge("WHIFF", JUDGE_COLORS.WHIFF)


func _on_track(_id: int, v: Dictionary) -> void:
	var pct := int(round(v.ratio * 100.0))
	_judge("TRACK %d%%" % pct, JUDGE_COLORS.PERFECT if v.ratio >= 0.5 else JUDGE_COLORS.MISS)


func _judge(text: String, color: Color) -> void:
	_judge_text = text
	_judge_color = color
	_judge_age = 0.0


func _process(delta: float) -> void:
	_judge_age += delta
	_beat_flash = maxf(_beat_flash - delta * 6.0, 0.0)
	_hurt = maxf(_hurt - delta * 3.0, 0.0)
	canvas.queue_redraw()


func _draw_hud() -> void:
	var size := canvas.size
	var c := size * 0.5
	var phase := DB.beat_phase()
	# crosshair dot + beat ring
	canvas.draw_circle(c, 2.5, Color(1, 1, 1, 0.95))
	var ring_r := lerpf(44.0, 8.0, phase)
	var ring_a := lerpf(0.25, 0.9, phase)
	canvas.draw_arc(c, ring_r, 0.0, TAU, 48, Color(0.15, 0.95, 1.0, ring_a), 2.0, true)
	canvas.draw_arc(c, 8.0, 0.0, TAU, 32, Color(1, 1, 1, 0.35 + 0.65 * _beat_flash), 1.5 + 2.5 * _beat_flash, true)
	# judgement popup
	if _judge_age < 0.8:
		var a := 1.0 - _judge_age / 0.8
		var rise := _judge_age * 30.0
		var col := Color(_judge_color, a)
		var w := _font.get_string_size(_judge_text, HORIZONTAL_ALIGNMENT_CENTER, -1, 30).x
		canvas.draw_string(_font, Vector2(c.x - w * 0.5, c.y + 90 - rise), _judge_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 30, col)
	# timing meter: 220 px for +-110 ms
	var mw := 220.0
	var my := c.y + 120
	var scale := mw / 220.0
	canvas.draw_rect(Rect2(c.x - mw * 0.5, my, mw, 6), Color(0.25, 0.25, 0.32, 0.9))
	canvas.draw_rect(Rect2(c.x - GOOD_MS * scale, my, GOOD_MS * 2 * scale, 6), Color(JUDGE_COLORS.GOOD, 0.5))
	canvas.draw_rect(Rect2(c.x - GREAT_MS * scale, my, GREAT_MS * 2 * scale, 6), Color(JUDGE_COLORS.GREAT, 0.6))
	canvas.draw_rect(Rect2(c.x - PERFECT_MS * scale, my, PERFECT_MS * 2 * scale, 6), Color(JUDGE_COLORS.PERFECT, 0.8))
	canvas.draw_line(Vector2(c.x, my - 4), Vector2(c.x, my + 10), Color(1, 1, 1, 0.9), 1.0)
	if _has_delta:
		var x := c.x + clampf(_last_delta, -110.0, 110.0) * scale
		canvas.draw_rect(Rect2(x - 1.5, my - 6, 3, 18), Color(1, 1, 1, 1))
		canvas.draw_string(_font, Vector2(c.x + mw * 0.5 + 10, my + 8), "%+.0f ms" % _last_delta, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.8))
	# HP bar bottom centre; red edge flash on damage
	var hp_w := 320.0
	var hp_y := size.y - 60
	canvas.draw_rect(Rect2(c.x - hp_w * 0.5, hp_y, hp_w, 10), Color(0.2, 0.2, 0.26, 0.9))
	var hp_col := Color(0.15, 0.95, 1.0) if _hp > 40 else Color(1.0, 0.35, 0.35)
	canvas.draw_rect(Rect2(c.x - hp_w * 0.5, hp_y, hp_w * _hp / 100.0, 10), hp_col)
	canvas.draw_string(_font, Vector2(c.x + hp_w * 0.5 + 10, hp_y + 10), "%d" % _hp, HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color.WHITE)
	if _hurt > 0.0:
		canvas.draw_rect(Rect2(0, 0, size.x, 6), Color(1.0, 0.25, 0.25, _hurt))
		canvas.draw_rect(Rect2(0, size.y - 6, size.x, 6), Color(1.0, 0.25, 0.25, _hurt))
	# Downbeat charge under the HP bar: fills with on-beat kills, READY at 16.
	if DB.game != null:
		var charge: int = DB.game.volley_charge()
		var cw := 160.0
		var cy := hp_y + 16
		canvas.draw_rect(Rect2(c.x - cw * 0.5, cy, cw, 4), Color(0.2, 0.2, 0.26, 0.9))
		canvas.draw_rect(Rect2(c.x - cw * 0.5, cy, cw * charge / 16.0, 4), Palette.YELLOW)
		if DB.game.volley_ready():
			var marked: int = (DB.game.marked() as Array).size()
			var txt := "DOWNBEAT READY   hold RMB to mark, release on the bar" if marked == 0 else "MARKED %d   release on the bar" % marked
			var tw := _font.get_string_size(txt, HORIZONTAL_ALIGNMENT_CENTER, -1, 14).x
			canvas.draw_string(_font, Vector2(c.x - tw * 0.5, cy + 18), txt, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Palette.YELLOW)
	# Ghost: a hollow crosshair where the ghost aims, score delta under yours.
	if _ghost != null and _ghost.active and DB.game != null:
		var aim: Array = _ghost.aim()
		var cam := get_viewport().get_camera_3d()
		if aim.size() == 2 and cam != null:
			var dir: Vector3 = Basis.from_euler(Vector3(float(aim[1]), float(aim[0]), 0.0)) * Vector3(0, 0, -1)
			var point: Vector3 = cam.global_position + dir * 20.0
			if not cam.is_position_behind(point):
				var p := cam.unproject_position(point)
				canvas.draw_arc(p, 9.0, 0.0, TAU, 24, Color(1, 1, 1, 0.55), 1.5, true)
				canvas.draw_line(p + Vector2(-14, 0), p + Vector2(-5, 0), Color(1, 1, 1, 0.55), 1.5)
				canvas.draw_line(p + Vector2(5, 0), p + Vector2(14, 0), Color(1, 1, 1, 0.55), 1.5)
		var delta := int(DB.game.score()) - _ghost.score()
		var behind: float = _ghost.behind_s()
		var gt := "GHOST %s   %d   %s%d%s" % [str(_ghost.ghost_name), _ghost.score(), "+" if delta >= 0 else "", delta, ("   ghost %.1f s behind" % behind) if behind > 0.1 else ""]
		canvas.draw_string(_font, Vector2(24, 128), gt, HORIZONTAL_ALIGNMENT_LEFT, -1, 16, JUDGE_COLORS.PERFECT if delta >= 0 else JUDGE_COLORS.MISS)
	# Boss bar top centre: the phase's kills against its clear line, the
	# boss's remaining parts, and the bar the phase closes on.
	if DB.game != null and DB.running:
		var b: Dictionary = DB.game.boss_status()
		if not b.is_empty() and not bool(b.dead) and float(DB.game.chart_time(DB.game.song_time())) >= float(_boss_start) - 8.0:
			var bw := 520.0
			var by := 70.0
			canvas.draw_rect(Rect2(c.x - bw * 0.5, by, bw, 14), Color(0.2, 0.2, 0.26, 0.9))
			var left: float = float(int(b.all_total) - int(b.all_kills)) / float(maxi(int(b.all_total), 1))
			canvas.draw_rect(Rect2(c.x - bw * 0.5, by, bw * left, 14), Palette.RED)
			var phase_frac: float = float(b.kills) / float(maxi(int(b.total), 1))
			canvas.draw_rect(Rect2(c.x - bw * 0.5, by + 16, bw * phase_frac, 5), Palette.CYAN)
			var clear_x: float = c.x - bw * 0.5 + bw * _phase_clear(int(b.phase))
			canvas.draw_line(Vector2(clear_x, by + 14), Vector2(clear_x, by + 23), Palette.YELLOW, 2.0)
			var bars_left: float = (float(b.phase_end_time) - DB.chart_time_now()) / (4.0 * 60.0 / maxf(DB.bpm, 1.0))
			var bt := "%s   phase %d/%d   %d/%d   %.1f bars" % [str(b.name).to_upper(), int(b.phase) + 1, int(b.phases), int(b.kills), int(b.total), maxf(bars_left, 0.0)]
			var btw := _font.get_string_size(bt, HORIZONTAL_ALIGNMENT_CENTER, -1, 15).x
			canvas.draw_string(_font, Vector2(c.x - btw * 0.5, by + 38), bt, HORIZONTAL_ALIGNMENT_LEFT, -1, 15, Color(1, 1, 1, 0.85))
	# segment strip top centre
	var seg := _segment_text()
	if not seg.is_empty():
		var sw := _font.get_string_size(seg, HORIZONTAL_ALIGNMENT_CENTER, -1, 18).x
		canvas.draw_string(_font, Vector2(c.x - sw * 0.5, 36), seg, HORIZONTAL_ALIGNMENT_LEFT, -1, 18, Color(1, 1, 1, 0.75))
	if DB.watching:
		var wt := "REPLAY   [R] play it yourself"
		var ww := _font.get_string_size(wt, HORIZONTAL_ALIGNMENT_CENTER, -1, 16).x
		canvas.draw_string(_font, Vector2(c.x - ww * 0.5, 60), wt, HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Palette.YELLOW)
	# score / combo
	if DB.game != null:
		canvas.draw_string(_font, Vector2(24, 40), "SCORE %d" % DB.game.score(), HORIZONTAL_ALIGNMENT_LEFT, -1, 26, Color.WHITE)
		var combo: int = DB.game.combo()
		if combo >= 2:
			canvas.draw_string(_font, Vector2(24, 76), "x%d" % combo, HORIZONTAL_ALIGNMENT_LEFT, -1, 34, JUDGE_COLORS.PERFECT)
		if not _best.is_empty():
			canvas.draw_string(_font, Vector2(24, 104), "BEST %d  %s" % [int(_best.get("score", 0)), str(_best.get("grade", ""))], HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(1, 1, 1, 0.6))
		var bar_text := "%.0f BPM   %.2f stars   %s   beat %.0f   tier %d   drift %+.1f ms   offset %+.0f ms   [Esc] menu  [Tab] range  [O] any song  [Q] dash  [RMB] downbeat  [R] restart  [C] calibrate" % [
			DB.bpm, _stars, str(DB.game.mods()), floor(DB.game.song_beat()), _tier, DB.game.drift_ms(), DB.input_offset_ms()]
		canvas.draw_string(_font, Vector2(24, size.y - 20), bar_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.6))
