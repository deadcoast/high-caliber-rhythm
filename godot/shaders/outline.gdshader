// Inverted-hull ink line, applied as next_pass on cel materials.
// Width is scaled by view distance so the line reads the same thickness on
// screen near and far. SCAFFOLD: the shipped line work is a depth+normal
// edge pass (ASSETS.register.md [SHD-02]); this hull only covers silhouettes.
shader_type spatial;
render_mode cull_front, unshaded, depth_draw_opaque;

uniform float width_px : hint_range(0.0, 8.0) = 2.5;
uniform vec4 ink : source_color = vec4(0.06, 0.03, 0.09, 1.0);

void vertex() {
	vec4 view_pos = MODELVIEW_MATRIX * vec4(VERTEX, 1.0);
	float dist = max(-view_pos.z, 0.1);
	float world_width = width_px * dist / (VIEWPORT_SIZE.y * PROJECTION_MATRIX[1][1]) * 2.0;
	VERTEX += NORMAL * world_width;
}

void fragment() {
	ALBEDO = ink.rgb;
}
