#version 100

precision mediump float;

varying vec2 fragTexCoord;
varying vec4 fragColor;

uniform sampler2D texture0;

uniform float u_time;
uniform float u_pulse;
uniform float u_pulse_y;

float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

vec3 sparkle_color(float seed) {
    float h = fract(seed * 7.13);
    if (h < 0.25) return vec3(1.00, 0.85, 0.30);
    if (h < 0.50) return vec3(1.00, 0.45, 0.65);
    if (h < 0.75) return vec3(0.45, 0.95, 1.00);
    return vec3(0.65, 1.00, 0.55);
}

void main() {
    vec2 uv = fragTexCoord;

    float ring_radius = (1.0 - u_pulse) * 0.7;
    float dist = abs(uv.y - u_pulse_y);
    float band = exp(-pow((dist - ring_radius) * 12.0, 2.0));

    float warp = sin(dist * 60.0 - u_time * 22.0) * band * 0.018 * u_pulse;
    uv.x += warp;

    vec3 col = texture2D(texture0, clamp(uv, 0.0, 1.0)).rgb;

    vec2 grid_res = vec2(80.0, 45.0);
    vec2 grid = floor(fragTexCoord * grid_res);
    float tick = floor(u_time * 16.0);
    float n = hash(grid + tick * 0.137);
    float zone = exp(-pow((dist - ring_radius) * 5.0, 2.0));
    float sparkle = step(0.93, n) * zone * u_pulse;
    col += sparkle_color(n + tick) * sparkle * 1.4;

    col += band * u_pulse * 0.22;

    gl_FragColor = vec4(col, 1.0);
}
