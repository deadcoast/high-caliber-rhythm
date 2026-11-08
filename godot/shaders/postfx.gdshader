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
uniform float hurt : hint_range(0.0, 1.0) = 0.0;
// Edgerunners post: chromatic split that breathes with the beat and spikes on
// damage, a faint scanline, and row glitch on damage.
uniform float aberration : hint_range(0.0, 1.0) = 0.5;
uniform float scanlines : hint_range(0.0, 1.0) = 0.25;
uniform float glitch : hint_range(0.0, 1.0) = 0.0;
uniform float time_s = 0.0;
uniform vec4 flash_color : source_color = vec4(0.15, 0.95, 1.0, 1.0);

float hash(float n) {
	return fract(sin(n) * 43758.5453);
}

void fragment() {
	vec2 uv = SCREEN_UV;
	// Row glitch: a few rows tear sideways while glitch > 0.
	if (glitch > 0.0) {
		float row = floor(uv.y * 48.0);
		float tear = step(0.82, hash(row + floor(time_s * 24.0))) * glitch;
		uv.x += (hash(row * 7.1 + time_s) - 0.5) * 0.06 * tear;
	}
	vec2 c = uv - 0.5;
	float r = dot(c, c) * 2.0;
	// Chromatic split grows toward the edges, pulses on the beat, spikes with damage.
	float split = (0.0025 + 0.002 * (1.0 - beat_phase) + 0.01 * hurt) * aberration * strength * (0.3 + r);
	vec3 col;
	col.r = texture(screen_tex, uv + c * split).r;
	col.g = texture(screen_tex, uv).g;
	col.b = texture(screen_tex, uv - c * split).b;
	// Scanlines: faint, 1 px period at 1080p.
	col *= 1.0 - scanlines * 0.12 * strength * step(0.5, fract(uv.y * 540.0));
	// Vignette tightens as the beat lands and relaxes after it.
	float breathe = 0.35 + 0.25 * (1.0 - beat_phase);
	float vig = smoothstep(0.45, 1.15, r) * breathe * strength;
	col *= 1.0 - vig;
	float l = dot(col, vec3(0.299, 0.587, 0.114));
	col = mix(col, vec3(l), grey * 0.6 * strength);
	col += flash_color.rgb * flash * 0.25 * strength * (1.0 - smoothstep(0.0, 0.9, r));
	// Damage: red pushes in from the edges, independent of post strength so it is always read.
	col = mix(col, vec3(0.9, 0.1, 0.1), hurt * 0.55 * smoothstep(0.2, 1.1, r));
	COLOR = vec4(col, 1.0);
}
