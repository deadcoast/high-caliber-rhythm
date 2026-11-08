// Toon ramp + rim. The lighting mechanism the art direction builds on:
// banded diffuse (Borderlands flat-ink shading) with a hard rim for the
// neon-lit silhouette read. Albedo/ramp/rim textures are art slots
// (ASSETS.register.md [SHD-01]); the uniforms below are the greybox values.
shader_type spatial;
render_mode cull_back, depth_draw_opaque;

uniform vec4 albedo : source_color = vec4(0.55, 0.55, 0.60, 1.0);
uniform float bands : hint_range(1.0, 8.0) = 3.0;
uniform float shadow_floor : hint_range(0.0, 1.0) = 0.28;
uniform float rim_threshold : hint_range(0.0, 1.0) = 0.68;
uniform float rim_strength : hint_range(0.0, 2.0) = 0.45;
uniform vec4 rim_color : source_color = vec4(0.15, 0.95, 1.0, 1.0);
uniform vec4 emission_color : source_color = vec4(1.0, 0.2, 0.7, 1.0);
uniform float emission_strength : hint_range(0.0, 8.0) = 0.0;
// Edgerunners read: two hard tones with a crisp cut, a small cel specular
// shape, and an optional painted ramp that replaces the banding entirely.
uniform bool two_tone = true;
uniform float tone_cut : hint_range(0.0, 1.0) = 0.42;
uniform float spec_size : hint_range(0.0, 1.0) = 0.08;
uniform float spec_strength : hint_range(0.0, 1.0) = 0.35;
uniform bool use_ramp = false;
uniform sampler2D ramp_tex : hint_default_white, repeat_disable;

void fragment() {
	ALBEDO = albedo.rgb;
	EMISSION = emission_color.rgb * emission_strength;
	ROUGHNESS = 1.0;
	SPECULAR = 0.0;
	METALLIC = 0.0;
}

void light() {
	float ndl = clamp(dot(NORMAL, LIGHT), 0.0, 1.0);
	float shade;
	if (use_ramp) {
		shade = texture(ramp_tex, vec2(ndl, 0.5)).r;
	} else if (two_tone) {
		shade = mix(shadow_floor, 1.0, step(tone_cut, ndl));
	} else {
		float stepped = floor(ndl * bands + 0.5) / bands;
		shade = shadow_floor + (1.0 - shadow_floor) * stepped;
	}
	DIFFUSE_LIGHT += ALBEDO * LIGHT_COLOR * ATTENUATION * shade;
	// Cel specular: a hard-edged highlight the size of spec_size.
	vec3 h = normalize(LIGHT + VIEW);
	float ndh = clamp(dot(NORMAL, h), 0.0, 1.0);
	float spec = step(1.0 - spec_size, ndh) * spec_strength;
	SPECULAR_LIGHT += LIGHT_COLOR * spec * ATTENUATION;
	float facing = clamp(dot(NORMAL, VIEW), 0.0, 1.0);
	float rim = step(rim_threshold, 1.0 - facing) * rim_strength;
	DIFFUSE_LIGHT += rim_color.rgb * rim * ATTENUATION;
}
