extends Node
## Online (autoload): the client of server/ (dbserver). Every finished run's
## replay is posted; the server re-scores it, inspects it and ranks it. The
## results screen shows the world rank and the world board; the menu has the
## daily; a board's top replay becomes a ghost. Settings in [online]: url
## (empty = offline), name, player_id and token (issued once by the server).

signal submitted(result: Dictionary)
signal board_received(fingerprint: String, mods: String, board: Dictionary)
signal daily_received(daily: Dictionary)
signal replay_received(id: int, json: String, meta: Dictionary)
signal profile_received(profile: Dictionary)
signal failed(what: String, message: String)

const TIMEOUT := 20.0

var url := ""
var player_name := "player"
var player_id := ""
var token := ""
var last_result: Dictionary = {}
var daily: Dictionary = {}
var daily_fetched_on := ""
var busy := 0


func _ready() -> void:
	url = str(Settings.get_value("online", "url")).strip_edges().trim_suffix("/")
	player_name = str(Settings.get_value("online", "name"))
	player_id = str(Settings.get_value("online", "player_id"))
	token = str(Settings.get_value("online", "token"))
	Settings.changed.connect(func(section: String, key: String, value: Variant) -> void:
		if section == "online" and key == "url":
			url = str(value).strip_edges().trim_suffix("/")
		elif section == "online" and key == "name":
			player_name = str(value))
	DB.run_finished.connect(_on_finished)


func configured() -> bool:
	return not url.is_empty()


## One HTTP call; `cb(status: int, body: Variant)` gets the parsed JSON (or the raw text).
func _request(method: int, path: String, body: Variant, cb: Callable, with_auth: bool = false) -> void:
	if not configured():
		cb.call(0, {"error": "offline: set online/url in settings"})
		return
	var req := HTTPRequest.new()
	req.timeout = TIMEOUT
	add_child(req)
	busy += 1
	var headers := PackedStringArray(["content-type: application/json", "accept: application/json"])
	if with_auth:
		headers.append("authorization: Bearer %s:%s" % [player_id, token])
	req.request_completed.connect(func(result: int, code: int, _h: PackedStringArray, bytes: PackedByteArray) -> void:
		busy -= 1
		req.queue_free()
		if result != HTTPRequest.RESULT_SUCCESS:
			cb.call(0, {"error": "connection failed (%d)" % result})
			return
		var text := bytes.get_string_from_utf8()
		var parsed = JSON.parse_string(text)
		cb.call(code, parsed if parsed != null else text))
	var data := "" if body == null else (body if body is String else JSON.stringify(body))
	var err := req.request(url + path, headers, method, data)
	if err != OK:
		busy -= 1
		req.queue_free()
		cb.call(0, {"error": "request failed (%d)" % err})


## Register once; `then` runs with true when a player id is in hand.
func ensure_player(then: Callable) -> void:
	if not player_id.is_empty() and not token.is_empty():
		then.call(true)
		return
	_request(HTTPClient.METHOD_POST, "/v1/players", {"name": player_name}, func(code: int, body: Variant) -> void:
		if code == 200 and body is Dictionary:
			player_id = str(body.id)
			token = str(body.token)
			Settings.set_value("online", "player_id", player_id)
			Settings.set_value("online", "token", token)
			then.call(true)
		else:
			failed.emit("register", str(body.get("error", body)) if body is Dictionary else str(body))
			then.call(false))


func _on_finished(_report: Dictionary) -> void:
	last_result = {}
	if not configured() or DB.watching or DB.calibrating or DB.last_replay_path.is_empty():
		return
	if not DB.current_drill.is_empty():
		return
	submit_replay(DB.last_replay_path, DB.daily_date)


## Post a saved replay; the server answers with its own score and the rank.
func submit_replay(path: String, daily_date: String = "") -> void:
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		failed.emit("submit", "replay not found: " + path)
		return
	ensure_player(func(ok: bool) -> void:
		if not ok:
			return
		var endpoint := "/v1/daily/scores" if not daily_date.is_empty() else "/v1/scores"
		# The replay text is sent as it was saved (the server parses it; the
		# view track rides along and is ignored by the core).
		var body := '{"replay":' + text + '}'
		_request(HTTPClient.METHOD_POST, endpoint, body, func(code: int, resp: Variant) -> void:
			if code == 200 and resp is Dictionary:
				last_result = resp
				submitted.emit(resp)
				fetch_board(str(resp.fingerprint), str(resp.mods))
			else:
				failed.emit("submit", str(resp.get("error", resp)) if resp is Dictionary else str(resp)), true))


func fetch_board(fingerprint: String, mods: String, limit: int = 10) -> void:
	_request(HTTPClient.METHOD_GET, "/v1/boards/%s?mods=%s&limit=%d" % [fingerprint, mods, limit], null, func(code: int, resp: Variant) -> void:
		if code == 200 and resp is Dictionary:
			board_received.emit(fingerprint, mods, resp)
		else:
			failed.emit("board", str(resp.get("error", resp)) if resp is Dictionary else str(resp)))


func fetch_daily() -> void:
	_request(HTTPClient.METHOD_GET, "/v1/daily", null, func(code: int, resp: Variant) -> void:
		if code == 200 and resp is Dictionary:
			daily = resp
			daily_fetched_on = str(resp.date)
			daily_received.emit(resp)
		else:
			failed.emit("daily", str(resp.get("error", resp)) if resp is Dictionary else str(resp)))


func fetch_daily_board() -> void:
	_request(HTTPClient.METHOD_GET, "/v1/daily/board", null, func(code: int, resp: Variant) -> void:
		if code == 200 and resp is Dictionary:
			board_received.emit("daily", str(resp.get("mods", "NM")), resp)
		else:
			failed.emit("daily board", str(resp.get("error", resp)) if resp is Dictionary else str(resp)))


## A stored replay by id (a board entry's `id`), for a ghost.
func fetch_replay(id: int, meta: Dictionary = {}) -> void:
	_request(HTTPClient.METHOD_GET, "/v1/replays/%d" % id, null, func(code: int, resp: Variant) -> void:
		if code == 200:
			var json: String = resp if resp is String else JSON.stringify(resp, "", false, true)
			replay_received.emit(id, json, meta)
		else:
			failed.emit("replay", str(resp.get("error", resp)) if resp is Dictionary else str(resp)))


func fetch_profile() -> void:
	if player_id.is_empty():
		return
	_request(HTTPClient.METHOD_GET, "/v1/players/" + player_id, null, func(code: int, resp: Variant) -> void:
		if code == 200 and resp is Dictionary:
			profile_received.emit(resp))


## Start today's daily: the level as the server charted it, with its mods.
func start_daily() -> void:
	if daily.is_empty():
		return
	DB.start_daily(JSON.stringify(daily.level, "", false, true), str(daily.mods), str(daily.date))
