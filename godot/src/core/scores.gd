extends Node
## Scores (autoload): local leaderboards, osu style. One board per chart
## fingerprint and mod set, top 10 by score with the replay path, pp, grade and
## FC flags. Best pp per chart feeds the profile rating. Persisted to
## user://scores.json.

const PATH := "user://scores.json"
const TOP := 10

signal new_best(level: String, entry: Dictionary)
signal rating_changed(rating: float)

var boards: Dictionary = {}
var profile_rating := 0.0


func _ready() -> void:
	var text := FileAccess.get_file_as_string(PATH)
	if not text.is_empty():
		var parsed = JSON.parse_string(text)
		if typeof(parsed) == TYPE_DICTIONARY and parsed.has("boards"):
			boards = parsed.boards
	DB.run_finished.connect(_on_finished)
	call_deferred("_recompute_rating")


func key(level: String, fingerprint: String, mods: String) -> String:
	return "%s@%s@%s" % [level, fingerprint, mods]


func board(level: String, fingerprint: String, mods: String) -> Array:
	return boards.get(key(level, fingerprint, mods), [])


func best(level: String, fingerprint: String, mods: String = "NM") -> Dictionary:
	var b := board(level, fingerprint, mods)
	return b[0] if not b.is_empty() else {}


func _on_finished(report: Dictionary) -> void:
	if DB.game == null or DB.calibrating or DB.watching:
		return
	var level := str(DB.game.level_name())
	var fp := str(DB.game.chart_fingerprint())
	var mods := str(report.get("mods", "NM"))
	var k := key(level, fp, mods)
	var integrity: Dictionary = DB.game.integrity()
	var entry := {
		"score": int(report.score),
		"pp": float(report.pp),
		"grade": str(report.grade),
		"fc": bool(report.fc),
		"pfc": bool(report.pfc),
		"max_combo": int(report.max_combo),
		"beat_accuracy": float(report.beat_accuracy),
		"aim_accuracy": float(report.aim_accuracy),
		"timing_std_ms": float(report.timing_std_ms),
		"downs": int(report.downs),
		"mods": mods,
		"stars": float(report.stars),
		"when": Time.get_datetime_string_from_system(),
		"replay": DB.last_replay_path,
		"clean": bool(integrity.get("clean", true)),
		"flags": integrity.get("flags", []),
	}
	var b: Array = boards.get(k, [])
	var prev_best := int(b[0].score) if not b.is_empty() else -1
	b.append(entry)
	b.sort_custom(func(x, y) -> bool: return int(x.score) > int(y.score))
	if b.size() > TOP:
		b = b.slice(0, TOP)
	boards[k] = b
	_save()
	if int(entry.score) > prev_best:
		new_best.emit(level, entry)
	_recompute_rating()


## Best pp per chart (any mods), weighted the osu way by the core.
func _recompute_rating() -> void:
	if DB.game == null:
		return
	var best_per_chart := {}
	for k in boards:
		var chart_key: String = k.get_slice("@", 0) + "@" + k.get_slice("@", 1)
		for e in boards[k]:
			if bool(e.get("clean", true)):
				best_per_chart[chart_key] = maxf(float(best_per_chart.get(chart_key, 0.0)), float(e.pp))
	var pps: Array[float] = []
	for v in best_per_chart.values():
		pps.append(float(v))
	profile_rating = float(DB.game.profile_rating(pps))
	rating_changed.emit(profile_rating)


func _save() -> void:
	var f := FileAccess.open(PATH, FileAccess.WRITE)
	if f == null:
		push_warning("scores: could not write " + PATH)
		return
	f.store_string(JSON.stringify({"boards": boards}))
	f.close()
