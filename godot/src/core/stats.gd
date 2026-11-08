extends Node
## Stats (autoload): every run's measurements, persisted to user://stats.json,
## with the Aim Lab-style views: per drill and BPM band, rank per band, the
## weakest cells, and a daily routine. The results and Range screens
## (ASSETS.register.md [HUD-06], [UI-01]) read from here.

const PATH := "user://stats.json"
const MAX_RUNS := 4000
const RANK_WINDOW := 20

signal recorded(run: Dictionary)

var runs: Array = []


func _ready() -> void:
	var text := FileAccess.get_file_as_string(PATH)
	if not text.is_empty():
		var parsed = JSON.parse_string(text)
		if typeof(parsed) == TYPE_DICTIONARY and parsed.has("runs"):
			runs = parsed.runs
	DB.run_finished.connect(_on_finished)


func _save() -> void:
	var f := FileAccess.open(PATH, FileAccess.WRITE)
	if f == null:
		push_warning("stats: could not write " + PATH)
		return
	f.store_string(JSON.stringify({"runs": runs}))
	f.close()


func _on_finished(report: Dictionary) -> void:
	if DB.game == null or DB.calibrating or DB.watching:
		return
	var run := {
		"when": Time.get_datetime_string_from_system(),
		"level": str(DB.game.level_name()),
		"drill": DB.current_drill,
		"bpm": DB.bpm,
		"band": str(DB.game.bpm_band(DB.bpm)),
		"composite": float(report.composite),
		"beat_accuracy": float(report.beat_accuracy),
		"aim_accuracy": float(report.aim_accuracy),
		"precision": float(report.precision),
		"timing_mean_ms": float(report.timing_mean_ms),
		"timing_std_ms": float(report.timing_std_ms),
		"track_ratio": float(report.track_ratio),
		"downs": int(report.downs),
		"grade": str(report.grade),
		"by_kind": report.by_kind,
	}
	runs.append(run)
	if runs.size() > MAX_RUNS:
		runs = runs.slice(runs.size() - MAX_RUNS)
	_save()
	recorded.emit(run)


func runs_for(drill: String, band: String) -> Array:
	var out: Array = []
	for r in runs:
		if (drill.is_empty() or str(r.get("drill", "")) == drill) and (band.is_empty() or str(r.get("band", "")) == band):
			out.append(r)
	return out


## Mean of the last `window` runs' fields for a drill/band cell.
func summary(drill: String, band: String, window: int = RANK_WINDOW) -> Dictionary:
	var rs := runs_for(drill, band)
	if rs.is_empty():
		return {"runs": 0}
	rs = rs.slice(maxi(rs.size() - window, 0))
	var acc := {"composite": 0.0, "beat_accuracy": 0.0, "aim_accuracy": 0.0, "precision": 0.0, "timing_std_ms": 0.0, "timing_mean_ms": 0.0}
	for r in rs:
		for k in acc:
			acc[k] += float(r.get(k, 0.0))
	for k in acc:
		acc[k] /= rs.size()
	acc["runs"] = rs.size()
	acc["last"] = rs[rs.size() - 1].when
	return acc


## Rank in a BPM band from the mean composite of the last runs across all drills.
func rank(band: String) -> String:
	var s := summary("", band)
	if int(s.runs) == 0:
		return "UNRANKED"
	return str(DB.game.rank_for(float(s.composite))) if DB.game != null else "?"


## Drill names in a band sorted weakest first; unplayed drills come first.
func weakest(band: String) -> Array[String]:
	var cells: Array = []
	for d in DB.game.drills():
		var s := summary(str(d.name), band)
		cells.append({"name": str(d.name), "runs": int(s.runs), "composite": float(s.get("composite", -1.0))})
	cells.sort_custom(func(a, b) -> bool:
		if a.runs == 0 and b.runs > 0:
			return true
		if b.runs == 0 and a.runs > 0:
			return false
		return a.composite < b.composite)
	var out: Array[String] = []
	for c in cells:
		out.append(c.name)
	return out


## Five drills for today: the weakest cells in this band.
func routine(band: String) -> Array[String]:
	var w := weakest(band)
	return w.slice(0, mini(5, w.size()))
