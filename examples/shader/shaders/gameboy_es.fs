#version 100

precision mediump float;

varying vec2 fragTexCoord;
varying vec4 fragColor;

uniform sampler2D texture0;

const vec3 PAL0 = vec3(0.059, 0.220, 0.059);
const vec3 PAL1 = vec3(0.188, 0.384, 0.188);
const vec3 PAL2 = vec3(0.545, 0.675, 0.059);
const vec3 PAL3 = vec3(0.608, 0.737, 0.059);

void main() {
    vec3 src = texture2D(texture0, fragTexCoord).rgb;
    float lum = dot(src, vec3(0.299, 0.587, 0.114));

    vec3 col;
    if (lum < 0.25)      col = PAL0;
    else if (lum < 0.50) col = PAL1;
    else if (lum < 0.75) col = PAL2;
    else                 col = PAL3;

    gl_FragColor = vec4(col, 1.0);
}
