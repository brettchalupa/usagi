#version 100

precision mediump float;

varying vec2 fragTexCoord;
varying vec4 fragColor;

uniform sampler2D texture0;

void main() {
    vec4 texel = texture2D(texture0, fragTexCoord);
    float lum = dot(texel.rgb, vec3(0.299, 0.587, 0.114));

    vec3 dark  = vec3(0.196, 0.184, 0.161);  // #322f29
    vec3 light = vec3(0.843, 0.831, 0.800);  // #d7d4cc

    gl_FragColor = vec4(mix(dark, light, lum), texel.a);
}
