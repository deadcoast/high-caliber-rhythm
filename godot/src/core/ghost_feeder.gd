class_name GhostFeeder
extends RefCounted
## Drives a second model with a replay's inputs in lockstep with the live
## clock: the same horizon rule as the driver's own watcher (DB._feed_replay),
## on another DbGame instance. The ghost's hits come back as events from its
## update; a restart inside the ghost's own run is honoured as the ghost
## falling behind.

var game = null
var inputs: Array = []
var index := 0
var advance_pending := false


func setup(ghost_game, replay_inputs: Array) -> void:
	game = ghost_game
	inputs = replay_inputs
	index = 0
	advance_pending = false


## Apply every input whose chart time falls inside the coming frame.
func feed(delta: float) -> void:
	if game == null:
		return
	var horizon: float = game.chart_time(game.song_time()) + delta * 1.25 + 0.002
	while index < inputs.size() and float(inputs[index].at) <= horizon:
		var inp: Dictionary = inputs[index]
		index += 1
		if str(inp.type) == "advance":
			advance_pending = true
		game.advance_judge(float(inp.at))
		match str(inp.type):
			"shot":
				game.fire(float(inp.raw_time), int(inp.target), float(inp.offset_m))
			"track":
				game.track_sample(int(inp.target), bool(inp.on))
			"mark":
				game.mark(int(inp.target))
			"release":
				game.release(float(inp.raw_time))
		horizon = game.chart_time(game.song_time()) + delta * 1.25 + 0.002


## The ghost's model restarted: expected after a recorded Advance, otherwise
## the frame crossed an expiry first and the queued inputs up to that Advance
## belong to the attempt that ended.
func on_restart() -> void:
	if advance_pending:
		advance_pending = false
		return
	while index < inputs.size():
		var was_advance := str(inputs[index].type) == "advance"
		index += 1
		if was_advance:
			break


func done() -> bool:
	return index >= inputs.size()
