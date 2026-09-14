#version 100
precision highp float;

varying vec2 v_px;

uniform vec2 u_center;
uniform vec2 u_size;
uniform float u_radius;
uniform float u_smoothing;

float sd_shape(vec2 p, vec2 half_size, float radius) {
    float r = clamp(radius * (1.0 + u_smoothing), 0.0, min(half_size.x, half_size.y));
    float n = 2.0 + 2.4 * u_smoothing;
    vec2 q = abs(p) - half_size + r;
    vec2 c = max(q, 0.0) / max(r, 1e-3);
    float corner = pow(pow(c.x, n) + pow(c.y, n), 1.0 / n) * r;
    return min(max(q.x, q.y), 0.0) + corner - r;
}

void main() {
    float sd = sd_shape(v_px - u_center, u_size * 0.5, u_radius);
    float coverage = clamp(0.5 - sd, 0.0, 1.0);
    if (coverage <= 0.0) discard;
    gl_FragColor = vec4(vec3(coverage), 1.0);
}
