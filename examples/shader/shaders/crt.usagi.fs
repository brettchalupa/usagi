#usagi shader 1

uniform float u_time;
uniform float u_scanline;
uniform vec2 u_resolution;

vec2 curve(vec2 uv) {
    uv = uv * 2.0 - 1.0;
    vec2 offset = abs(uv.yx) / vec2(8.0, 6.0);
    uv = uv + uv * offset * offset;
    return uv * 0.5 + 0.5;
}

vec4 usagi_main(vec2 uv, vec4 color) {
    vec2 curved_uv = curve(uv);
    if (curved_uv.x < 0.0 || curved_uv.x > 1.0 || curved_uv.y < 0.0 || curved_uv.y > 1.0) {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }

    float ca = 0.0015;
    vec3 col;
    col.r = usagi_texture(texture0, curved_uv + vec2(ca, 0.0)).r;
    col.g = usagi_texture(texture0, curved_uv).g;
    col.b = usagi_texture(texture0, curved_uv - vec2(ca, 0.0)).b;

    float scan = sin(curved_uv.y * u_resolution.y * 3.14159 * 2.0);
    col *= 1.0 - u_scanline * 0.4 * (0.5 - 0.5 * scan);

    vec2 v = uv - 0.5;
    float vig = 1.0 - dot(v, v) * 1.2;
    col *= clamp(vig, 0.0, 1.0);

    col *= 0.97 + 0.03 * sin(u_time * 6.0 + curved_uv.y * 8.0);

    return vec4(col, 1.0) * color;
}
