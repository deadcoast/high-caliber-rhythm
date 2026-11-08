extends Node3D
## Entry point. With no flags the menu opens (level select); flags start a run.
##   godot --path godot -- --bpm=140          the street level (act one, level one) at 140
##   godot --path godot -- --hard --mods=DTHR Street Hard with double time and hard rock
##   godot --path godot -- --range=switch --bpm=150 --minutes=2   a Range drill (Tab in game opens the Range)
##   godot --path godot -- --warmup --bpm=174 the Range warmup drill
##   godot --path godot -- --fill --bpm=150   the Range fill drill (every kind incl. RUSH)
##   godot --path godot -- --level=res://levels/warmup_128.json
##   godot --path godot -- --autoplay           the bot plays (attract mode / glue check)
##   godot --headless --path godot -- --smoke --autoplay    run to the end, print the report JSON, quit
##   godot --headless --path godot -- --replay=user://replays/x.json    re-score a saved run, print, quit
##   godot --path godot -- --watch=user://replays/x.json    watch a saved run (camera follows the recording)
##   godot --path godot -- --song=C:/music/track.mp3  Any Song without the confirm screen: analyse, build, play
##   godot --headless --path godot -- --list-levels     print every level with rating and best, quit

@export var bpm := 128.0


func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	var level := ""
	var replay := ""
	var watch := ""
	var song := ""
	var drill := ""
	var minutes := 2.0
	var hard := "--hard" in args
	var asked := false
	for a in args:
		if a.begins_with("--"):
			asked = true
		if a.begins_with("--bpm="):
			bpm = float(a.trim_prefix("--bpm="))
		elif a.begins_with("--level="):
			level = a.trim_prefix("--level=")
		elif a.begins_with("--replay="):
			replay = a.trim_prefix("--replay=")
		elif a.begins_with("--watch="):
			watch = a.trim_prefix("--watch=")
		elif a.begins_with("--song="):
			song = a.trim_prefix("--song=")
		elif a.begins_with("--range="):
			drill = a.trim_prefix("--range=")
		elif a.begins_with("--minutes="):
			minutes = float(a.trim_prefix("--minutes="))
		elif a.begins_with("--mods="):
			DB.mods = a.trim_prefix("--mods=").to_upper()
	if "--list-levels" in args:
		Levels.print_index()
		get_tree().quit()
		return
	if not replay.is_empty():
		print("DOWNBEAT REPLAY " + JSON.stringify(DB.replay_report(replay)))
		get_tree().quit()
		return
	if "--autoplay" in args:
		var bot := Node.new()
		bot.name = "Autoplay"
		bot.set_script(load("res://src/core/autoplay.gd"))
		add_child(bot)
	if "--smoke" in args:
		DB.run_finished.connect(func(r: Dictionary) -> void:
			var summary := r.duplicate()
			summary.erase("history")
			print("DOWNBEAT SMOKE " + JSON.stringify(summary))
			get_tree().quit())
	if not watch.is_empty():
		if not DB.watch_replay(watch):
			get_tree().quit(1)
	elif not song.is_empty():
		DB.skip_confirm = true
		DB.song_analysed.connect(func(path: String, name: String, a: Dictionary) -> void:
			DB.skip_confirm = false
			DB.build_any_song(path, name, float(a.bpm), float(a.offset_s), a), Object.CONNECT_ONE_SHOT)
		DB.analyse_song(song)
	elif not level.is_empty():
		DB.start_level(level)
	elif not drill.is_empty():
		DB.start_drill(drill, bpm, minutes)
	elif "--fill" in args:
		DB.start_fill(bpm)
	elif "--warmup" in args:
		DB.start_warmup(bpm)
	elif hard:
		DB.start_street_hard(bpm)
	elif asked:
		DB.start_street(bpm)
	else:
		var menu := get_node_or_null("Menu")
		if menu != null:
			menu.open_screen(menu.PLAY)
		else:
			DB.start_street(bpm)
