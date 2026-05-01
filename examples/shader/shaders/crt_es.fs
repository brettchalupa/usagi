#version 100

precision mediump float;

varying vec2 fragTexCoord;
varying vec4 fragColor;

uniform sampler2D texture0;

uniform float u_time;
uniform float u_scanline;
uniform vec2 u_resolution;

vec2 curve(vec2 uv) {
    uv = uv * 2.0 - 1.0;
    vec2 offset = abs(uv.yx) / vec2(8.0, 6.0);
    uv = uv + uv * offset * offset;
    return uv * 0.5 + 0.5;
}

void main() {
    vec2 uv = curve(fragTexCoord);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    float ca = 0.0015;
    vec3 col;
    col.r = texture2D(texture0, uv + vec2(ca, 0.0)).r;
    col.g = texture2D(texture0, uv).g;
    col.b = texture2D(texture0, uv - vec2(ca, 0.0)).b;

    float scan = sin(uv.y * u_resolution.y * 3.14159 * 2.0);
    col *= 1.0 - u_scanline * 0.4 * (0.5 - 0.5 * scan);

    vec2 v = (fragTexCoord - 0.5);
    float vig = 1.0 - dot(v, v) * 1.2;
    col *= clamp(vig, 0.0, 1.0);

    col *= 0.97 + 0.03 * sin(u_time * 6.0 + uv.y * 8.0);

    gl_FragColor = vec4(col, 1.0);
}
