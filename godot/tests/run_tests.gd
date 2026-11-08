extends SceneTree
## Headless engine-side tests: the Rust binding through GDScript, the click
## layers, and every shipped level. Run with
##   godot --headless --path godot --script res://tests/run_tests.gd
## (tools/build.ps1 -Test / tools/build.sh --test). Exit code 1 on failure.

var _fails := 0
var _count := 0


func _init() -> void:
	print("DOWNBEAT engine tests")
	_test_binding_present()
	if ClassDB.class_exists("DbGame"):
		_test_binding_round_trip()
		_test_fill_level()
		_test_bot_through_binding()
		_test_levels_compile()
		_test_replay_round_trip()
	_test_click_layers()
	_test_kit()
	print("%d checks, %d failed" % [_count, _fails])
	quit(1 if _fails > 0 else 0)


func _check(name: String, ok: bool, detail: String = "") -> void:
	_count += 1
	if ok:
		print("  ok    " + name)
	else:
		_fails += 1
		print("  FAIL  " + name + ("   " + detail if not detail.is_empty() else ""))


func _test_binding_present() -> void:
	_check("DbGame class registered", ClassDB.class_exists("DbGame"), "run tools/build.ps1 first")


func _test_binding_round_trip() -> void:
	var g = ClassDB.instantiate("DbGame")
	_check("load_warmup", g.load_warmup(128.0, 0.0, 0.0))
	_check("bpm", is_equal_approx(g.bpm(), 128.0))
	_check("target_count", g.target_count() == 70, str(g.target_count()))
	var t: Dictionary = g.target(0)
	_check("target dict keys", t.has("kind") and t.has("hit_time") and t.has("radius") and t.has("yaw"), str(t.keys()))
	_check("first target at beat 8", is_equal_approx(float(t.hit_beat), 8.0))
	var p: Vector3 = g.target_position_at(0, float(t.hit_time))
	_check("target position on the lane", p.z < 0.0 and absf(p.length() - 12.0) < 0.01, str(p))
	var r: Dictionary = g.rating()
	_check("rating anchor", absf(float(r.stars) - 3.0) < 0.02, str(r))
	_check("fingerprint", str(g.chart_fingerprint()).length() == 16)


func _test_fill_level() -> void:
	var g = ClassDB.instantiate("DbGame")
	_check("load_fill", g.load_fill(150.0, 0.0, 0.0))
	var kinds := {}
	for i in g.target_count():
		kinds[g.target(i).kind] = true
	_check("fill has RUSH targets", kinds.has("RUSH"), str(kinds.keys()))
	_check("fill drum_stem default -1", int(g.drum_stem()) == -1)
	_check("fill kit_paths empty", (g.kit_paths() as Dictionary).is_empty())
	_check("fill rates above warmup", float(g.rating().stars) > 3.0, str(g.rating()))


func _test_kit() -> void:
	var kit := Kit.build()
	_check("kit has every voice", kit.size() == 6 and kit.has("RUSH") and kit.has("DEAD"), str(kit.keys()))
	for kind in kit:
		var w: AudioStreamWAV = kit[kind]
		var frames := w.data.size() / 2
		_check("kit %s non-silent" % kind, frames > 1000 and _rms(w.data, 0, mini(frames, 400)) > 0.02, str(frames))


func _test_bot_through_binding() -> void:
	var g = ClassDB.instantiate("DbGame")
	g.load_warmup(140.0, 0.0, 0.0)
	g.start(-1.0)
	var fired := {}
	var ticks := 0
	var spawns := 0
	var spawned: Array[int] = []
	var tracks := {}
	var t := -1.0
	var dt := 1.0 / 240.0
	var total: float = g.chart_total_time() + 0.5
	while t < total:
		var events: Array = g.update(dt, false, 0.0)
		t += dt
		for e in events:
			match e.type:
				"tick":
					ticks += 1
					for id in tracks:
						g.track_sample(id, true)
				"spawn":
					spawns += 1
					spawned.append(int(e.target))
					if g.target(e.target).kind == "TRACK":
						tracks[e.target] = true
				"track_done":
					tracks.erase(e.target)
		for id in spawned:
			if fired.has(id):
				continue
			var info: Dictionary = g.target(id)
			if info.kind != "TRACK" and float(info.hit_time) <= g.chart_time(t):
				fired[id] = true
				var v: Dictionary = g.fire(float(info.hit_time), id, 0.0)
				if v.type != "hit" or v.timing != "PERFECT":
					_check("shot %d perfect" % id, false, str(v))
	var rep: Dictionary = g.report()
	_check("bot: all targets spawned", spawns == 70, str(spawns))
	_check("bot: finished", g.is_finished())
	_check("bot: perfect == hits == 68", int(rep.perfect) == 68 and int(rep.hits) == 68, "%s/%s" % [rep.perfect, rep.hits])
	_check("bot: no expiries, no whiffs", int(rep.expired) == 0 and int(rep.whiffs) == 0, "%s %s" % [rep.expired, rep.whiffs])
	_check("bot: track 100%", is_equal_approx(float(rep.track_ratio), 1.0), str(rep.track_ratio))
	_check("bot: grade S", str(rep.grade) == "S", str(rep.grade))
	_check("bot: history length", (rep.history as Array).size() == 68)
	_check("bot: ticks ~ sixteenths", ticks > 0 and absi(ticks - int((total + 1.0) * 140.0 / 60.0 * 4.0)) <= 2, str(ticks))


func _test_replay_round_trip() -> void:
	var g = ClassDB.instantiate("DbGame")
	g.load_warmup(128.0, 0.0, 0.0)
	g.start(-1.0)
	var t := -1.0
	while t < 12.0:
		g.update(1.0 / 120.0, false, 0.0)
		t += 1.0 / 120.0
		for id in 4:
			var info: Dictionary = g.target(id)
			if float(info.hit_time) <= g.chart_time(t) and float(info.hit_time) > g.chart_time(t) - 1.0 / 120.0:
				g.fire(float(info.hit_time) + 0.02, id, 0.1)
	var json: String = g.replay_json()
	_check("replay json non-empty", json.length() > 100)
	var rep: Dictionary = g.replay_report(json)
	_check("replay report has score", rep.has("score") and int(rep.hits) == 4, str(rep.get("hits")))


func _test_levels_compile() -> void:
	var g = ClassDB.instantiate("DbGame")
	var dir := DirAccess.open("res://levels")
	_check("levels dir", dir != null)
	if dir == null:
		return
	for f in dir.get_files():
		if not f.ends_with(".json"):
			continue
		var info: Dictionary = g.rate_level_json(FileAccess.get_file_as_string("res://levels/" + f))
		_check("level %s compiles (%.2f stars)" % [f, float(info.get("stars", 0.0))], not info.is_empty() and float(info.get("stars", 0.0)) > 0.0)


func _test_click_layers() -> void:
	var layers := ClickTrack.build_layers(120.0, 8)
	_check("three layers", layers.size() == 3)
	var n := layers[0].data.size()
	_check("layers equal length", layers[1].data.size() == n and layers[2].data.size() == n)
	var spb := 0.5
	var sr := ClickTrack.SAMPLE_RATE
	var pulse_on_beat := _rms(layers[0].data, int(1.0 * spb * sr), 400)
	var pulse_off_beat := _rms(layers[0].data, int(1.25 * spb * sr), 400)
	var hats_off_beat := _rms(layers[1].data, int(1.5 * spb * sr), 200)
	var hats_on_beat := _rms(layers[1].data, int(1.0 * spb * sr), 200)
	var accent_bar := _rms(layers[2].data, int(4.0 * spb * sr), 200)
	var accent_beat := _rms(layers[2].data, int(1.0 * spb * sr), 200)
	_check("pulse sits on the beat", pulse_on_beat > 0.1 and pulse_off_beat < 0.001, "%f %f" % [pulse_on_beat, pulse_off_beat])
	_check("hats sit on the off-beat", hats_off_beat > 0.02 and hats_on_beat < 0.001, "%f %f" % [hats_off_beat, hats_on_beat])
	_check("accent only on bar starts", accent_bar > 0.05 and accent_beat < 0.001, "%f %f" % [accent_bar, accent_beat])


func _rms(data: PackedByteArray, frame: int, count: int) -> float:
	var acc := 0.0
	for i in count:
		var v := data.decode_s16((frame + i) * 2) / 32767.0
		acc += v * v
	return sqrt(acc / count)
