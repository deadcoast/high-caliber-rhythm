class_name Cel
extends RefCounted
## Material factory for the greybox. Every mesh in the arena goes through here
## so the toon ramp and ink line are applied uniformly.

static var _cel_shader: Shader
static var _outline_shader: Shader


static func material(color: Color, emission := 0.0, emission_color := Color(1.0, 0.2, 0.7), outline := true) -> ShaderMaterial:
	if _cel_shader == null:
		_cel_shader = load("res://shaders/cel.gdshader")
		_outline_shader = load("res://shaders/outline.gdshader")
	var m := ShaderMaterial.new()
	m.shader = _cel_shader
	m.set_shader_parameter("albedo", color)
	m.set_shader_parameter("emission_color", emission_color)
	m.set_shader_parameter("emission_strength", emission)
	if outline:
		var o := ShaderMaterial.new()
		o.shader = _outline_shader
		m.next_pass = o
	return m
