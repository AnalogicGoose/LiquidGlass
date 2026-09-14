#version 100
precision highp float;

varying vec2 v_uv;

// Textura de origen: la que se pasa a draw_texture_ex.
uniform sampler2D Texture;

uniform vec2 u_texel;     // 1 / tamaño de la textura de origen
uniform vec2 u_direction; // (1, 0) pasada horizontal, (0, 1) pasada vertical
uniform float u_sigma;    // desviación del Gaussiano, en texels de origen

// Muestras a cada lado del centro. Se reparten sobre ±3 sigma, así que la
// separación entre muestras nunca pasa de ~1 texel dentro del rango del slider.
const int HALF_TAPS = 32;

void main() {
    if (u_sigma < 0.05) {
        gl_FragColor = vec4(texture2D(Texture, v_uv).rgb, 1.0);
        return;
    }

    float spacing = 3.0 * u_sigma / float(HALF_TAPS);
    vec2 step_uv = u_direction * u_texel * spacing;

    vec3 sum = vec3(0.0);
    float weight_sum = 0.0;

    for (int i = -HALF_TAPS; i <= HALF_TAPS; i++) {
        float x = float(i) * spacing;
        float w = exp(-0.5 * x * x / (u_sigma * u_sigma));
        sum += texture2D(Texture, v_uv + step_uv * float(i)).rgb * w;
        weight_sum += w;
    }

    gl_FragColor = vec4(sum / weight_sum, 1.0);
}
