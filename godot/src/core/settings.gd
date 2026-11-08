extends Node
## Settings (autoload): persisted to user://downbeat.cfg. Everything a player
## tunes lives here so calibration, sensitivity and post-FX strength survive
## restarts. The settings menu (ASSETS.register.md [UI-01]) edits these values.

const PATH := "user://downbeat.cfg"

signal changed(section: String, key: String, value: Variant)

var _cfg := ConfigFile.new()

const DEFAULTS := {
	"input": {"cm_per_360": 34.0, "dpi": 800.0, "offset_ms": 0.0},
	"video": {"fov": 80.0, "postfx": 0.6, "max_fps": 0},
	"audio": {"master_db": 0.0},
}


func _ready() -> void:
	var err := _cfg.load(PATH)
	if err != OK and err != ERR_FILE_NOT_FOUND:
		push_warning("settings: could not read %s (%d), using defaults" % [PATH, err])
	for section in DEFAULTS:
		for key in DEFAULTS[section]:
			if not _cfg.has_section_key(section, key):
				_cfg.set_value(section, key, DEFAULTS[section][key])
	Engine.max_fps = int(get_value("video", "max_fps"))
	AudioServer.set_bus_volume_db(0, float(get_value("audio", "master_db")))


func get_value(section: String, key: String) -> Variant:
	return _cfg.get_value(section, key, DEFAULTS[section][key])


func set_value(section: String, key: String, value: Variant) -> void:
	_cfg.set_value(section, key, value)
	changed.emit(section, key, value)
	save()


func save() -> void:
	var err := _cfg.save(PATH)
	if err != OK:
		push_warning("settings: could not write %s (%d)" % [PATH, err])


func reset() -> void:
	_cfg = ConfigFile.new()
	_ready()
	save()
