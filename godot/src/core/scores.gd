extends Node
## Scores (autoload): best run per level, persisted to user://scores.cfg.
## A best is keyed by level name and chart fingerprint, so a changed chart
## starts a fresh table instead of comparing against runs on different targets.

const PATH := "user://scores.cfg"

signal new_best(level: String, report: Dictionary)

var _cfg := ConfigFile.new()


func _ready() -> void:
	var err := _cfg.load(PATH)
	if err != OK and err != ERR_FILE_NOT_FOUND:
		push_warning("scores: could not read %s (%d)" % [PATH, err])
	DB.run_finished.connect(_on_finished)


func _key(level: String, fingerprint: String) -> String:
	return "%s@%s" % [level, fingerprint]


func best(level: String, fingerprint: String) -> Dictionary:
	var section := _key(level, fingerprint)
	if not _cfg.has_section(section):
		return {}
	var out := {}
	for k in _cfg.get_section_keys(section):
		out[k] = _cfg.get_value(section, k)
	return out


func _on_finished(report: Dictionary) -> void:
	if DB.game == null or DB.calibrating:
		return
	var level := str(DB.game.level_name())
	var fp := str(DB.game.chart_fingerprint())
	var prev := best(level, fp)
	var prev_score := int(prev.get("score", -1))
	var score := int(report.get("score", 0))
	_cfg.set_value(_key(level, fp), "plays", int(prev.get("plays", 0)) + 1)
	if score > prev_score:
		var section := _key(level, fp)
		_cfg.set_value(section, "score", score)
		_cfg.set_value(section, "grade", str(report.get("grade", "")))
		_cfg.set_value(section, "max_combo", int(report.get("max_combo", 0)))
		_cfg.set_value(section, "beat_accuracy", float(report.get("beat_accuracy", 0.0)))
		_cfg.set_value(section, "aim_accuracy", float(report.get("aim_accuracy", 0.0)))
		_cfg.set_value(section, "timing_std_ms", float(report.get("timing_std_ms", 0.0)))
		_cfg.set_value(section, "when", Time.get_datetime_string_from_system())
		_cfg.set_value(section, "replay", DB.last_replay_path)
		new_best.emit(level, report)
	var err := _cfg.save(PATH)
	if err != OK:
		push_warning("scores: could not write %s (%d)" % [PATH, err])
