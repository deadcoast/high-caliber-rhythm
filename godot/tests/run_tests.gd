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
		_test_street_level()
		_test_damage_through_binding()
		_test_drills_and_restart()
		_test_mods_volley_integrity()
		_test_bot_through_binding()
		_test_levels_compile()
		_test_replay_round_trip()
		_test_watch_replay()
		_test_any_song_pipeline()
		_test_level_select_data()
		_test_boss_and_cues()
		_test_ghost_lockstep()
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
	var rooms: Array = g.rooms()
	_check("range room", rooms.size() == 1 and str(rooms[0].name) == "range", str(rooms))
	var stand: Vector3 = rooms[0].stand
	var p: Vector3 = g.target_position_at(0, float(t.hit_time))
	var rel := p - (stand + Vector3(0, 1.6, 0))
	_check("target 12 m down the lane from the stand", rel.z < 0.0 and absf(Vector2(rel.x, rel.z).length() - 12.0) < 0.05, str(rel))
	_check("target pos matches target()", (t.pos as Vector3).is_equal_approx(p))
	var r: Dictionary = g.rating()
	_check("rating anchor", absf(float(r.stars) - 3.0) < 0.02, str(r))
	_check("fingerprint", str(g.chart_fingerprint()).length() == 16)


func _test_street_level() -> void:
	var g = ClassDB.instantiate("DbGame")
	_check("load_street", g.load_street(128.0, 0.0, 0.0))
	var rooms: Array = g.rooms()
	_check("three rooms in path order", rooms.size() == 3 and str(rooms[0].name) == "street" and str(rooms[2].name) == "platform", str(rooms.size()))
	var prev_z := 0.0
	var ordered := true
	for r in rooms:
		var o: Vector3 = r.origin
		if o.z >= prev_z:
			ordered = false
		prev_z = o.z
	_check("rooms descend along -Z", ordered)
	var spans: Array = g.spans()
	_check("spans: traversal first, encounters present", spans.size() == 7 and str(spans[0].kind) == "traversal" and str(spans[1].kind) == "encounter", str(spans.size()))
	var t: Dictionary = g.target(0)
	_check("target carries room/enemy/damage/pos", t.has("room") and t.has("enemy") and int(t.damage) > 0 and t.has("pos"), str(t.keys()))
	_check("hp starts at 100", int(g.hp()) == 100)


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


func _test_damage_through_binding() -> void:
	# Nobody shoots: every enemy fires; damage and down events arrive in order.
	var g = ClassDB.instantiate("DbGame")
	g.load_warmup(128.0, 0.0, 0.0)
	g.start(0.0)
	var damages := 0
	var downs := 0
	var last_hp := 100
	var ok := true
	var t := 0.0
	var total: float = g.chart_total_time() + 0.5
	while t < total:
		for e in g.update(1.0 / 60.0, false, 0.0):
			if e.type == "damage":
				damages += 1
				if int(e.hp) >= last_hp and last_hp != 100:
					ok = false
				last_hp = int(e.hp)
			elif e.type == "down":
				downs += 1
				last_hp = 100
		t += 1.0 / 60.0
	var rep: Dictionary = g.report()
	_check("damage events for every non-track enemy", damages + downs == 68, "%d + %d" % [damages, downs])
	_check("downs recorded", downs >= 5 and int(rep.downs) == downs, str(downs))
	_check("hp decreases between downs", ok)


func _test_drills_and_restart() -> void:
	var g = ClassDB.instantiate("DbGame")
	var ds: Array = g.drills()
	_check("drill catalogue", ds.size() >= 6 and ds[0].has("name") and ds[0].has("pass_beats"), str(ds.size()))
	_check("load_drill switch 150 x 1 min", g.load_drill("switch", 150.0, 1.0, 0.0))
	_check("drill duration ~ 1 min", absf(float(g.chart_total_time()) - 60.0) < 30.0, str(g.chart_total_time()))
	_check("rank_for / bpm_band", str(g.rank_for(0.9)) == "PLATINUM" and str(g.bpm_band(150.0)) == "140-159")
	# Nobody shoots a drill: the down restarts it; a restart event arrives and spawns repeat.
	g.set_no_fail(false)
	g.load_drill("gridshot", 128.0, 1.0, 0.0)
	g.start(0.0)
	var restarts := 0
	var spawns := 0
	var t := 0.0
	while t < 40.0 and restarts < 2:
		for e in g.update(1.0 / 60.0, false, 0.0):
			if e.type == "restart":
				restarts += 1
				t = float(e.chart_time)
			elif e.type == "spawn":
				spawns += 1
		t += 1.0 / 60.0
	_check("down restarts the section", restarts == 2 and int(g.restarts()) == 2, str(restarts))
	_check("spawns repeat after restart", spawns > int(g.target_count()) * 0 + 10, str(spawns))
	_check("hp refilled after restart", int(g.hp()) == 100, str(g.hp()))


func _test_mods_volley_integrity() -> void:
	var g = ClassDB.instantiate("DbGame")
	g.set_mods("DTHR")
	g.load_warmup(128.0, 0.0, 0.0)
	_check("mods applied: DT bpm", is_equal_approx(float(g.bpm()), 192.0), str(g.bpm()))
	_check("mods label", str(g.mods()) == "DTHR")
	_check("mod multiplier", absf(float(g.mod_multiplier()) - 1.12 * 1.06) < 1e-6)
	var meta: Dictionary = g.level_meta()
	_check("level_meta", meta.has("song") and meta.has("difficulty"))
	g.set_mods("NM")
	_check("load_street_hard", g.load_street_hard(128.0, 0.0, 0.0) and str(g.level_meta().difficulty) == "Hard")
	# Volley through the binding: kill 16 on the beat, mark, release on the bar.
	g.load_warmup(120.0, 0.0, 0.0)
	g.start(0.0)
	var killed := 0
	var t := 0.0
	var dt := 1.0 / 120.0
	while killed < 16 and t < 30.0:
		g.update(dt, false, 0.0)
		t += dt
		var ct: float = g.chart_time(t)
		for id in g.target_count():
			var info: Dictionary = g.target(id)
			if info.kind != "TRACK" and float(info.hit_time) <= ct and float(info.hit_time) > ct - dt:
				var v: Dictionary = g.fire(float(info.hit_time), id, 0.0)
				if v.type == "hit":
					killed += 1
	_check("volley charged by 16 on-beat kills", bool(g.volley_ready()) and int(g.volley_charge()) == 16, str(g.volley_charge()))
	# Advance until two switch targets are live, mark them, release on the next bar.
	var live: Array = []
	while live.size() < 2 and t < 40.0:
		g.update(dt, false, 0.0)
		t += dt
		live.clear()
		for id in g.target_count():
			var info: Dictionary = g.target(id)
			if info.kind == "SWITCH" and float(info.spawn_time) <= g.chart_time(t) and float(info.hit_time) + 0.05 > g.chart_time(t):
				live.append(id)
	var marks := 0
	for id in live:
		if g.mark(id):
			marks += 1
	_check("marks accepted", marks >= 2, str(marks))
	var bar_time := ceilf(g.chart_time(t) / 2.0) * 2.0
	var v: Dictionary = g.release(bar_time + 0.01)
	_check("volley kills on the bar", str(v.type) == "volley" and (v.kills as Array).size() == marks, str(v))
	_check("charge spent", int(g.volley_charge()) == 0)
	var rep: Dictionary = g.report()
	_check("report has pp/fc/mods", rep.has("pp") and rep.has("fc") and str(rep.mods) == "NM" and float(rep.pp) > 0.0)
	var integ: Dictionary = g.integrity()
	_check("integrity dict", integ.has("flags") and integ.has("suspicion"))
	var pps: Array[float] = [100.0, 100.0]
	var pr: float = g.profile_rating(pps)
	_check("profile_rating weights 0.95", absf(pr - 195.0) < 1e-6, str(pr))


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


## Watching: load_replay rebuilds the game from a saved run and
## replay_inputs hands back the inputs; feeding them through advance_judge +
## fire at their recorded times ends on the live report.
func _test_watch_replay() -> void:
	var g = ClassDB.instantiate("DbGame")
	g.load_warmup(128.0, 0.0, 0.0)
	g.start(-1.0)
	var t := -1.0
	while not g.is_finished() and t < 60.0:
		g.update(1.0 / 120.0, false, 0.0)
		t += 1.0 / 120.0
		for id in 70:
			var info: Dictionary = g.target(id)
			var ht := float(info.hit_time)
			if ht <= g.chart_time(t) and ht > g.chart_time(t) - 1.0 / 120.0 and id % 3 != 2:
				g.fire(ht + 0.01, id, 0.05)
	var live: Dictionary = g.report()
	var json: String = g.replay_json()
	var w = ClassDB.instantiate("DbGame")
	_check("load_replay", w.load_replay(json))
	var inputs: Array = w.replay_inputs()
	_check("replay_inputs count", inputs.size() >= int(live.shots) and inputs.size() > 20, "%d vs %d shots" % [inputs.size(), int(live.shots)])
	_check("replay input keys", inputs.size() > 0 and inputs[0].has("at") and inputs[0].has("raw_time") and inputs[0].has("target"), str(inputs[0] if inputs.size() > 0 else {}))
	w.start(-1.0)
	var index := 0
	var wt := -1.0
	var dt := 1.0 / 60.0
	while not w.is_finished() and wt < 60.0:
		var horizon: float = w.chart_time(wt) + dt * 1.25 + 0.002
		while index < inputs.size() and float(inputs[index].at) <= horizon:
			var inp: Dictionary = inputs[index]
			index += 1
			w.advance_judge(float(inp.at))
			if str(inp.type) == "shot":
				w.fire(float(inp.raw_time), int(inp.target), float(inp.offset_m))
		w.update(dt, false, 0.0)
		wt += dt
	var watched: Dictionary = w.report()
	_check("every input applied", index == inputs.size(), "%d/%d" % [index, inputs.size()])
	_check("watched report matches live", int(watched.score) == int(live.score) and int(watched.hits) == int(live.hits) and int(watched.expired) == int(live.expired) and is_equal_approx(float(watched.timing_std_ms), float(live.timing_std_ms)),
		"%d/%d hits %d/%d expired %d/%d" % [int(watched.score), int(live.score), int(watched.hits), int(live.hits), int(watched.expired), int(live.expired)])
	_check("load_replay refuses garbage", not w.load_replay("{\"nope\": 1}"))


## Any Song: a synthetic 120 BPM click rendered to a wav, analysed through
## the binding, regridded from the cache, decoded for playback, and turned
## into a level that compiles.
func _test_any_song_pipeline() -> void:
	var g = ClassDB.instantiate("DbGame")
	var click := ClickTrack.build(120.0, 40)
	var path := "user://_test_song.wav"
	var ok := click.save_to_wav(path)
	_check("test wav written", ok == OK)
	if ok != OK:
		return
	var os_path := ProjectSettings.globalize_path(path)
	var a: Dictionary = g.analyze_file(os_path)
	_check("analyze_file returns a grid", a.has("bpm") and a.has("offset_s") and a.has("energy") and a.has("onset_grid"), str(a.keys()))
	if a.is_empty():
		return
	var bpm := float(a.bpm)
	var folded := bpm
	while folded > 170.0:
		folded *= 0.5
	while folded < 85.0:
		folded *= 2.0
	_check("tempo found (%.2f)" % bpm, absf(folded - 120.0) < 1.0)
	var r: Dictionary = g.regrid_file(os_path, 120.0, float(a.offset_s))
	_check("regrid from cache", r.has("energy") and int(r.bars) >= 8 and (r.onset_grid as Array).size() == 64, str(r.get("bars")))
	var d: Dictionary = g.decode_file(os_path)
	_check("decode_file pcm", d.has("pcm") and int(d.sample_rate) == 44100 and int(d.channels) == 1 and (d.pcm as PackedByteArray).size() == click.data.size(), str(d.get("sample_rate")))
	var c: Dictionary = g.grid_click(120.0, 0.25, 4.0, 44100)
	var pcm: PackedByteArray = c.pcm
	_check("grid_click length", pcm.size() == 4 * 44100 * 2)
	_check("grid_click on the beat", _rms(pcm, int(0.25 * 44100), 400) > 0.1 and _rms(pcm, int(0.45 * 44100), 400) < 0.001)
	var energy: Array[float] = []
	for v in r.energy:
		energy.append(float(v))
	var json: String = g.auto_level_json("test song", os_path, 120.0, float(a.offset_s), float(r.duration_s), energy)
	var info: Dictionary = g.rate_level_json(json)
	_check("auto level compiles", not info.is_empty() and float(info.get("stars", 0.0)) > 0.0, str(info))
	var g2 = ClassDB.instantiate("DbGame")
	_check("auto level loads", g2.load_level_json(json, float(a.offset_s), 0.0))
	_check("auto level keeps the song path", str(g2.audio_path()) == os_path, str(g2.audio_path()))
	DirAccess.remove_absolute(os_path)


## Level select: builtin levels as JSON at any BPM, rated under a mod set
## with the leaderboard fingerprint for that mod set.
func _test_level_select_data() -> void:
	var g = ClassDB.instantiate("DbGame")
	for name in ["street", "street_hard", "warmup", "fill"]:
		var json: String = g.builtin_level_json(name, 140.0)
		_check("builtin_level_json %s" % name, json.length() > 100)
		var nm: Dictionary = g.rate_level_json_mods(json, "NM")
		var dt: Dictionary = g.rate_level_json_mods(json, "DTHR")
		_check("%s rates under mods" % name, not nm.is_empty() and not dt.is_empty() and str(nm.fingerprint) != str(dt.fingerprint) and float(dt.stars) > float(nm.stars), "%s %s" % [str(nm.get("stars")), str(dt.get("stars"))])
		_check("%s mods label echoed" % name, str(dt.get("mods")) == "DTHR" and is_equal_approx(float(nm.bpm), 140.0))
	_check("builtin_level_json unknown is empty", str(g.builtin_level_json("nope", 128.0)).is_empty())
	var night := FileAccess.get_file_as_string("res://levels/night_market_132.json")
	var info: Dictionary = g.rate_level_json_mods(night, "NM")
	_check("night market charted (%.2f stars, %d enemies)" % [float(info.get("stars", 0.0)), int(info.get("targets", 0))], int(info.get("targets", 0)) > 300 and int(info.get("stems", 0)) == 4 and str(info.get("song")) == "Night Market")


## The boss through the binding: phases, cues, boss_status, phase_end events,
## and an exact bot clearing all three phases.
func _test_boss_and_cues() -> void:
	var g = ClassDB.instantiate("DbGame")
	var text := FileAccess.get_file_as_string("res://levels/skyline_168.json")
	_check("skyline loads", g.load_level_json(text, 0.0, 0.0))
	var phases: Array = g.phases()
	_check("three boss phases", phases.size() == 3 and bool(phases[2].last) and not bool(phases[0].last), str(phases.size()))
	_check("phase rooms and clears", str(phases[0].room) == "helipad" and is_equal_approx(float(phases[2].clear), 0.8))
	var cues: Array = g.cues()
	_check("cues in time order", cues.size() == 6 and float(cues[0].time) <= float(cues[1].time) and str(cues[1].kind) == "reveal")
	var spans: Array = g.spans()
	var boss_spans := 0
	for sp in spans:
		if str(sp.kind) == "boss":
			boss_spans += 1
	_check("boss spans carry phase and label", boss_spans == 3 and str(spans[spans.size() - 2].label) == "the core")
	var b0: Dictionary = g.boss_status()
	_check("boss_status before the fight", not b0.is_empty() and int(b0.phase) == 0 and int(b0.all_total) > 200 and not bool(b0.dead), str(b0))
	g.start(-1.0)
	var t := -1.0
	var phase_events := 0
	var cue_events := 0
	var boss_parts := 0
	var tracks: Array[int] = []
	while not g.is_finished() and t < 200.0:
		for e in g.update(1.0 / 120.0, false, 0.0):
			match str(e.type):
				"phase_end":
					phase_events += 1
					_check("phase %d cleared by the exact bot" % int(e.phase), bool(e.cleared), "%d/%d" % [int(e.kills), int(e.total)])
				"cue":
					cue_events += 1
				"spawn":
					var info: Dictionary = g.target(int(e.target))
					if int(info.phase) >= 0:
						boss_parts += 1
					if str(info.kind) != "TRACK":
						g.fire(float(info.hit_time), int(e.target), 0.0)
					else:
						tracks.append(int(e.target))
				"track_done":
					tracks.erase(int(e.target))
				"tick":
					# The beam held on every strider: one sample per sixteenth, as the spawner does.
					for id in tracks:
						g.track_sample(id, true)
		t += 1.0 / 120.0
	var rep: Dictionary = g.report()
	_check("three phase_end events", phase_events == 3, str(phase_events))
	_check("six cues fired", cue_events == 6, str(cue_events))
	_check("boss parts spawned", boss_parts > 200, str(boss_parts))
	_check("report counts the phases", int(rep.phases_cleared) == 3 and int(rep.phase_fails) == 0, "%d %d" % [int(rep.phases_cleared), int(rep.phase_fails)])
	_check("boss dead at the end", bool(g.boss_status().dead))
	_check("boss bonus paid", int(rep.score) > 3000)


## Ghosts: a second model fed a replay through GhostFeeder in lockstep with
## a live run ends on the replay's own report (hits flash on the same ids).
func _test_ghost_lockstep() -> void:
	# A live run to make the replay (same bot as the watch test).
	var live = ClassDB.instantiate("DbGame")
	live.load_warmup(128.0, 0.0, 0.0)
	live.start(-1.0)
	var t := -1.0
	while not live.is_finished() and t < 60.0:
		live.update(1.0 / 120.0, false, 0.0)
		t += 1.0 / 120.0
		for id in 70:
			var info: Dictionary = live.target(id)
			var ht := float(info.hit_time)
			if ht <= live.chart_time(t) and ht > live.chart_time(t) - 1.0 / 120.0 and id % 4 != 3:
				live.fire(ht + 0.012, id, 0.08)
	var json: String = live.replay_json()
	var want: Dictionary = live.report()
	# The ghost races a fresh live model at a different frame rate.
	var ghost = ClassDB.instantiate("DbGame")
	_check("ghost loads the replay", ghost.load_replay(json))
	var feeder := GhostFeeder.new()
	feeder.setup(ghost, ghost.replay_inputs())
	ghost.start(-1.0)
	var hits := 0
	var ids: Array[int] = []
	var frames := 0
	while not ghost.is_finished() and frames < 60 * 90:
		var dt := 1.0 / 60.0 if frames % 50 != 0 else 0.09
		feeder.feed(dt)
		for e in ghost.update(dt, false, 0.0):
			if str(e.type) == "hit":
				hits += 1
				ids.append(int(e.target))
			elif str(e.type) == "restart":
				feeder.on_restart()
		frames += 1
	var got: Dictionary = ghost.report()
	_check("ghost fed every input", feeder.done())
	_check("ghost hits match the replay", hits == int(want.hits), "%d vs %d" % [hits, int(want.hits)])
	_check("ghost report matches the live report", int(got.score) == int(want.score) and int(got.expired) == int(want.expired) and is_equal_approx(float(got.timing_std_ms), float(want.timing_std_ms)), "%d vs %d" % [int(got.score), int(want.score)])
	_check("ghost killed the charted ids", ids.size() > 40 and not ids.has(3) and ids.has(0))


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
