extends CanvasLayer
## Beat-reactive post-FX layer (below the HUD). Reads the event bus and the
## beat phase, drives shaders/postfx.gdshader. Strength comes from Settings.

var rect: ColorRect
var mat: ShaderMaterial
var _flash := 0.0
var _grey := 0.0


func _ready() -> void:
	layer = 0
	rect = ColorRect.new()
	rect.set_anchors_preset(Control.PRESET_FULL_RECT)
	rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	mat = ShaderMaterial.new()
	mat.shader = load("res://shaders/postfx.gdshader")
	rect.material = mat
	add_child(rect)
	_apply_strength(float(Settings.get_value("video", "postfx")))
	Settings.changed.connect(func(section: String, key: String, value: Variant) -> void:
		if section == "video" and key == "postfx":
			_apply_strength(float(value)))
	DB.target_hit.connect(_on_hit)
	DB.combo_broken.connect(func() -> void: _grey = 1.0)


func _apply_strength(v: float) -> void:
	mat.set_shader_parameter("strength", v)
	rect.visible = v > 0.0


func _on_hit(_id: int, v: Dictionary) -> void:
	if v.timing == "PERFECT":
		_flash = 1.0
	elif v.timing == "GREAT":
		_flash = 0.5


func _process(delta: float) -> void:
	if not rect.visible:
		return
	_flash = maxf(_flash - delta * 8.0, 0.0)
	_grey = maxf(_grey - delta * 3.0, 0.0)
	mat.set_shader_parameter("beat_phase", DB.beat_phase())
	mat.set_shader_parameter("flash", _flash)
	mat.set_shader_parameter("grey", _grey)
