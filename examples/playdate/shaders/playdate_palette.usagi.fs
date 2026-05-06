#usagi shader 1

vec4 usagi_main(vec2 uv, vec4 color) {
    vec4 texel = usagi_texture(texture0, uv);
    float lum = dot(texel.rgb, vec3(0.299, 0.587, 0.114));

    vec3 dark = vec3(0.196, 0.184, 0.161);
    vec3 light = vec3(0.843, 0.831, 0.800);

    return vec4(mix(dark, light, lum), texel.a);
}
