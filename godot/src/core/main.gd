extends Node3D
## Entry point. Starts the warmup run at the chosen BPM.
##   godot --path godot -- --bpm=174        run the warmup at 174
##   godot --path godot -- --fill --bpm=150 run the fill chart (every kind incl. RUSH)
##   godot --path godot -- --level=res://levels/warmup_128.json
##   godot --path godot -- --autoplay           the bot plays (attract mode / glue check)
##   godot --headless --path godot -- --smoke --autoplay    run to the end, print the report JSON, quit
##   godot --headless --path godot -- --replay=user://replays/x.json    re-score a saved run, print, quit
##   godot --headless --path godot -- --list-levels     print every level with rating and best, quit

@export var bpm := 128.0


func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	var level := ""
	var replay := ""
	for a in args:
		if a.begins_with("--bpm="):
			bpm = float(a.trim_prefix("--bpm="))
		elif a.begins_with("--level="):
			level = a.trim_prefix("--level=")
		elif a.begins_with("--replay="):
			replay = a.trim_prefix("--replay=")
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
	if not level.is_empty():
		DB.start_level(level)
	elif "--fill" in args:
		DB.start_fill(bpm)
	else:
		DB.start_warmup(bpm)
