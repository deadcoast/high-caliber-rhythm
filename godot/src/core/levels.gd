class_name Levels
extends RefCounted
## Level index: every res://levels/*.json with its rating, for level select
## and the `--list-levels` printout.

const DIR := "res://levels"


static func index() -> Array[Dictionary]:
	var out: Array[Dictionary] = []
	if DB.game == null:
		return out
	var dir := DirAccess.open(DIR)
	if dir == null:
		return out
	var names := dir.get_files()
	names.sort()
	for f in names:
		if not f.ends_with(".json"):
			continue
		var path := "%s/%s" % [DIR, f]
		var info: Dictionary = DB.game.rate_level_json(FileAccess.get_file_as_string(path))
		if info.is_empty():
			continue
		info["path"] = path
		var b := Scores.best(str(info.name), str(info.fingerprint))
		info["best"] = b
		out.append(info)
	return out


## Designed console listing; no emoji, fixed columns.
static func print_index() -> void:
	var rows := index()
	print("%-28s %6s %6s %5s %7s %8s %6s  %s" % ["LEVEL", "BPM", "STARS", "BARS", "TARGETS", "BEST", "GRADE", "AUDIO"])
	for r in rows:
		var b: Dictionary = r.best
		print("%-28s %6.0f %6.2f %5d %7d %8s %6s  %s" % [
			str(r.name), float(r.bpm), float(r.stars), int(r.bars), int(r.targets),
			str(b.get("score", "-")), str(b.get("grade", "-")), str(r.audio) if not str(r.audio).is_empty() else "click"])
	print("%d level(s)" % rows.size())
