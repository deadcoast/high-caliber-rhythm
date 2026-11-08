extends Node
## Campaign (autoload): act one's levels in order, what the player has
## cleared, and what that unlocks. Persisted to user://campaign.json. A level
## unlocks when the one before it was cleared with grade C or better (call:
## mine, one number). An act is clear when its last level is.

const PATH := "user://campaign.json"
const UNLOCK_GRADES := ["S", "A", "B", "C"]

const ACTS := [
	{
		"name": "ACT ONE",
		"levels": [
			{"path": "res://levels/night_market_132.json", "name": "night_market_132", "title": "NIGHT MARKET"},
			{"path": "res://levels/undertow_150.json", "name": "undertow_150", "title": "UNDERTOW"},
			{"path": "res://levels/skyline_168.json", "name": "skyline_168", "title": "SKYLINE"},
		],
	},
]

signal progressed(level: String, entry: Dictionary)
signal act_cleared(act: int)

## level name -> {grade, score, pp, cleared, when, plays}
var progress: Dictionary = {}


func _ready() -> void:
	var text := FileAccess.get_file_as_string(PATH)
	if not text.is_empty():
		var parsed = JSON.parse_string(text)
		if typeof(parsed) == TYPE_DICTIONARY and parsed.has("progress"):
			progress = parsed.progress
	DB.run_finished.connect(_on_finished)


func level_index(level_name: String) -> Array:
	for a in ACTS.size():
		var levels: Array = ACTS[a].levels
		for i in levels.size():
			if str(levels[i].name) == level_name:
				return [a, i]
	return []


func is_campaign_level(level_name: String) -> bool:
	return not level_index(level_name).is_empty()


func is_cleared(level_name: String) -> bool:
	return bool(progress.get(level_name, {}).get("cleared", false))


## A level is open when it is first in its act or the one before it is cleared.
func is_unlocked(act: int, index: int) -> bool:
	if index == 0:
		return true
	var levels: Array = ACTS[act].levels
	return is_cleared(str(levels[index - 1].name))


func act_is_clear(act: int) -> bool:
	var levels: Array = ACTS[act].levels
	return is_cleared(str(levels[levels.size() - 1].name))


func best(level_name: String) -> Dictionary:
	return progress.get(level_name, {})


func _on_finished(report: Dictionary) -> void:
	if DB.game == null or DB.calibrating or DB.watching:
		return
	var name := str(DB.game.level_name())
	var idx := level_index(name)
	if idx.is_empty():
		return
	var grade := str(report.grade)
	var cleared := grade in UNLOCK_GRADES
	var prev: Dictionary = progress.get(name, {})
	var entry := {
		"grade": grade if _grade_rank(grade) >= _grade_rank(str(prev.get("grade", "F"))) else str(prev.get("grade")),
		"score": maxi(int(report.score), int(prev.get("score", 0))),
		"pp": maxf(float(report.pp), float(prev.get("pp", 0.0))),
		"cleared": cleared or bool(prev.get("cleared", false)),
		"when": Time.get_datetime_string_from_system(),
		"plays": int(prev.get("plays", 0)) + 1,
		"mods": str(report.get("mods", "NM")),
	}
	var was_act_clear := act_is_clear(int(idx[0]))
	progress[name] = entry
	_save()
	progressed.emit(name, entry)
	if not was_act_clear and act_is_clear(int(idx[0])):
		act_cleared.emit(int(idx[0]))


func _grade_rank(g: String) -> int:
	return ["F", "D", "C", "B", "A", "S"].find(g)


func _save() -> void:
	var f := FileAccess.open(PATH, FileAccess.WRITE)
	if f == null:
		push_warning("campaign: could not write " + PATH)
		return
	f.store_string(JSON.stringify({"progress": progress}))
	f.close()


func reset() -> void:
	progress = {}
	_save()
