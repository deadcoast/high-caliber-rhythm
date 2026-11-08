extends CanvasLayer
## Greybox HUD. Crosshair ring collapses onto the dot on every beat, judgement
## pops under it, timing meter shows where the last shot landed in the window.
## All typography, colour and layout are placeholders (ASSETS.register.md [HUD-*]).

const PERFECT_MS := 30.0
const GREAT_MS := 60.0
const GOOD_MS := 100.0

const JUDGE_COLORS := {
	"PERFECT": Color(0.15, 0.95, 1.0),
	"GREAT": Color(0.65, 1.0, 0.25),
	"GOOD": Color(1.0, 0.85, 0.2),
	"EARLY": Color(1.0, 0.35, 0.35),
	"LATE": Color(1.0, 0.35, 0.35),
	"WHIFF": Color(0.6, 0.6, 0.7),
	"MISS": Color(1.0, 0.2, 0.2),
}

var canvas: Control
var results: Label
var _judge_text := ""
var _judge_color := Color.WHITE
var _judge_age := 9.0
var _last_delta := 0.0
var _has_delta := false
var _beat_flash := 0.0
var _font: Font
var _tier := 0
var _report: Dictionary = {}
var _best: Dictionary = {}
var _stars := 0.0


func _ready() -> void:
	canvas = Control.new()
	canvas.set_anchors_preset(Control.PRESET_FULL_RECT)
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	canvas.draw.connect(_draw_hud)
	add_child(canvas)
	results = Label.new()
	results.set_anchors_preset(Control.PRESET_FULL_RECT)
	results.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	results.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	results.mouse_filter = Control.MOUSE_FILTER_IGNORE
	results.visible = false
	results.add_theme_font_size_override("font_size", 22)
	add_child(results)
	_font = ThemeDB.fallback_font
	DB.target_hit.connect(_on_hit)
	DB.whiff.connect(_on_whiff)
	DB.target_expired.connect(func(_id: int) -> void: _judge("MISS", JUDGE_COLORS.MISS))
	DB.track_done.connect(_on_track)
	DB.beat.connect(func(_i: int, _b: int) -> void: _beat_flash = 1.0)
	DB.run_started.connect(_on_started)
	DB.run_finished.connect(_on_finished)
	DB.calibrated.connect(func(ms: float) -> void: _judge("OFFSET %+.0f ms" % ms, JUDGE_COLORS.PERFECT))
	DB.tier_changed.connect(func(t: int) -> void: _tier = t)
	Scores.new_best.connect(func(_level: String, _r: Dictionary) -> void: _judge("NEW BEST", JUDGE_COLORS.PERFECT))


func _on_started(_bpm: float) -> void:
	results.visible = false
	_has_delta = false
	_report = {}
	if DB.game != null:
		_best = Scores.best(str(DB.game.level_name()), str(DB.game.chart_fingerprint()))
		_stars = float(DB.game.rating().get("stars", 0.0))


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
	if results.visible:
		_draw_scatter(c)
	# score / combo
	if DB.game != null:
		canvas.draw_string(_font, Vector2(24, 40), "SCORE %d" % DB.game.score(), HORIZONTAL_ALIGNMENT_LEFT, -1, 26, Color.WHITE)
		var combo: int = DB.game.combo()
		if combo >= 2:
			canvas.draw_string(_font, Vector2(24, 76), "x%d" % combo, HORIZONTAL_ALIGNMENT_LEFT, -1, 34, JUDGE_COLORS.PERFECT)
		if not _best.is_empty():
			canvas.draw_string(_font, Vector2(24, 104), "BEST %d  %s" % [int(_best.get("score", 0)), str(_best.get("grade", ""))], HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(1, 1, 1, 0.6))
		var bar_text := "%.0f BPM   %.2f stars   beat %.0f   stems tier %d   drift %+.1f ms   offset %+.0f ms   [R] restart  [C] calibrate  [Esc] mouse" % [
			DB.bpm, _stars, floor(DB.game.song_beat()), _tier, DB.game.drift_ms(), DB.input_offset_ms()]
		canvas.draw_string(_font, Vector2(24, size.y - 20), bar_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.6))


## Timing scatter for the results: one dot per judged shot, x = chart beat,
## y = ms from the beat. Reads the report history (HUD-06 backend).
func _draw_scatter(c: Vector2) -> void:
	var hist: Array = _report.get("history", [])
	if hist.is_empty():
		return
	var w := 640.0
	var h := 180.0
	var origin := Vector2(c.x - w * 0.5, c.y + 170)
	var scale_y := h * 0.5 / 110.0
	canvas.draw_rect(Rect2(origin, Vector2(w, h)), Color(0.08, 0.07, 0.12, 0.85))
	canvas.draw_rect(Rect2(origin.x, origin.y + h * 0.5 - GOOD_MS * scale_y, w, GOOD_MS * 2 * scale_y), Color(JUDGE_COLORS.GOOD, 0.12))
	canvas.draw_rect(Rect2(origin.x, origin.y + h * 0.5 - GREAT_MS * scale_y, w, GREAT_MS * 2 * scale_y), Color(JUDGE_COLORS.GREAT, 0.12))
	canvas.draw_rect(Rect2(origin.x, origin.y + h * 0.5 - PERFECT_MS * scale_y, w, PERFECT_MS * 2 * scale_y), Color(JUDGE_COLORS.PERFECT, 0.15))
	canvas.draw_line(Vector2(origin.x, origin.y + h * 0.5), Vector2(origin.x + w, origin.y + h * 0.5), Color(1, 1, 1, 0.5), 1.0)
	var first: float = hist[0].hit_beat
	var last: float = maxf(hist[hist.size() - 1].hit_beat, first + 1.0)
	for shot in hist:
		var x: float = origin.x + (float(shot.hit_beat) - first) / (last - first) * w
		var y: float = origin.y + h * 0.5 - clampf(float(shot.delta_ms), -110.0, 110.0) * scale_y
		canvas.draw_circle(Vector2(x, y), 3.0, JUDGE_COLORS.get(shot.timing, Color.WHITE))
	canvas.draw_string(_font, Vector2(origin.x, origin.y - 6), "timing by beat   early below, late above   +-110 ms", HORIZONTAL_ALIGNMENT_LEFT, -1, 13, Color(1, 1, 1, 0.6))


func _on_finished(r: Dictionary) -> void:
	_report = r
	var lines := [
		"GRADE %s     SCORE %d     MAX COMBO %d" % [r.grade, r.score, r.max_combo],
		"",
		"PERFECT %d   GREAT %d   GOOD %d   EARLY %d   LATE %d   MISS %d   WHIFF %d" % [r.perfect, r.great, r.good, r.early, r.late, r.expired, r.whiffs],
		"",
		"aim   %.1f%% hit   precision %.0f%%" % [r.aim_accuracy * 100.0, r.precision * 100.0],
		"beat  %.1f%% on-beat   quality %.0f%%   offset %+.1f ms   sd %.1f ms" % [r.beat_accuracy * 100.0, r.timing_quality * 100.0, r.timing_mean_ms, r.timing_std_ms],
		"track %.0f%% on-target" % [r.track_ratio * 100.0],
		"",
		"[R] run again",
	]
	results.text = "\n".join(lines)
	results.visible = true
