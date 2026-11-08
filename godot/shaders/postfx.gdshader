// Beat-reactive post pass: vignette breathe on the beat phase, a flash on
// Perfect, a desaturate pulse on a combo break. Strength is the player's
// setting (video/postfx) and 0 turns the pass off. SCAFFOLD look
// (ASSETS.register.md [SHD-03]); the hooks and the strength clamp are final.
shader_type canvas_item;

uniform sampler2D screen_tex : hint_screen_texture, filter_linear;
uniform float strength : hint_range(0.0, 1.0) = 0.6;
uniform float beat_phase : hint_range(0.0, 1.0) = 0.0;
uniform float flash : hint_range(0.0, 1.0) = 0.0;
uniform float grey : hint_range(0.0, 1.0) = 0.0;
uniform vec4 flash_color : source_color = vec4(0.15, 0.95, 1.0, 1.0);

void fragment() {
	vec3 col = texture(screen_tex, SCREEN_UV).rgb;
	vec2 c = SCREEN_UV - 0.5;
	float r = dot(c, c) * 2.0;
	// Vignette tightens as the beat lands and relaxes after it.
	float breathe = 0.35 + 0.25 * (1.0 - beat_phase);
	float vig = smoothstep(0.45, 1.15, r) * breathe * strength;
	col *= 1.0 - vig;
	float l = dot(col, vec3(0.299, 0.587, 0.114));
	col = mix(col, vec3(l), grey * 0.6 * strength);
	col += flash_color.rgb * flash * 0.25 * strength * (1.0 - smoothstep(0.0, 0.9, r));
	COLOR = vec4(col, 1.0);
}
