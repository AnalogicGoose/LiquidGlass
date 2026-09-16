#version 100
precision highp float;

// Réplica del componente de Figma "Liquid Glass - Regular - Large", que está
// dividido en dos capas apiladas:
//
//   1. "Fill + Shadow" (abajo): sombra proyectada, contorno fino y el tinte,
//      blanco (variante "Regular") o negro (variante "Dark").
//   2. "Glass Effect" (arriba): efecto GLASS de Figma (refracción, profundidad,
//      dispersión, frost, luz) y dos inner shadows que iluminan los bordes
//      superior e inferior. Su relleno blanco en Multiply no altera el color.

varying vec2 v_px;

uniform sampler2D u_scene;      // escena + cristales anteriores, resolución completa
uniform sampler2D u_scene_blur; // stack acumulado a media resolución, sigma = frost / 2
uniform sampler2D u_stack_mask; // cobertura y transición interna con frost
uniform vec2 u_resolution;

// Forma
uniform vec2 u_center;
uniform vec2 u_size;
uniform float u_radius;
uniform float u_smoothing; // corner smoothing de Figma (0 = circular, 0.6 = iOS)

// Capa "Glass Effect"
uniform float u_refraction;      // 0..1
uniform float u_depth;           // px: hasta dónde entra la curvatura del borde
uniform float u_dispersion;      // 0..1
uniform float u_frost;           // px: radio de desenfoque
uniform float u_light_intensity; // 0..1
uniform float u_light_angle;     // grados
uniform float u_splay;           // 0..1: cuánto se abre el brillo hacia dentro

// Capa "Fill + Shadow"
uniform float u_tint_mode; // 0 = tinte blanco (Liquid Glass Light), 1 = negro (Dark)
uniform float u_tint;      // multiplica la opacidad de los rellenos del tinte (1 = Figma)
uniform float u_shadow;    // multiplica la opacidad de la sombra (1 = Figma)

// Phase 8.5/8.8 (docs/SparkGlass_ROADMAP.md): color grade + adaptive knobs.
// All four are no-ops at their `preset()` defaults (1, 0, 1, 0, 0), so a
// surface that never sets them renders exactly as before this stage existed.
uniform float u_saturation;       // 1 = no change
uniform float u_brightness;       // 0 = no change
uniform float u_contrast;         // 1 = no change
uniform float u_clear_dimming;    // 0 = no local dimming (Clear Material Dimming, 8.8)
uniform float u_adaptive_response; // 0 = no backdrop-adaptive rim boost (8.5)
uniform float u_ambient_reflection; // 0 = rim glow color is fixed, not sampled from the backdrop (8.5)

// Phase 9.5 (docs/SparkGlass_ROADMAP.md): "Interaction Illumination" — a
// surface's edge glows while it's being touched/dragged/pressed. 0 (Idle,
// see glass.rs's interaction_energy()) is a no-op.
uniform float u_interaction;

const float IOR = 1.5;
const vec3 INTERACTION_GLOW_COLOR = vec3(1.0, 0.98, 0.9); // warm white — reads as "lit", not "tinted"

// Valores fijos del diseño de Figma. Los que cambian entre la variante clara
// ("Regular") y la oscura ("Dark") van en pares LIGHT / DARK.
const vec3 LIGHT_TINT_DARKEN_COLOR = vec3(0.749); // #BFBFBF
const vec3 DARK_TINT_COLOR = vec3(0.102);         // #1A1A1A
const float LIGHT_SHADOW_OPACITY = 0.25;
const float DARK_SHADOW_OPACITY = 0.45;
const float LIGHT_SHADOW_OFFSET = 8.0;
const float DARK_SHADOW_OFFSET = 18.0;
const float SHADOW_SIGMA = 24.0; // blur 48
const vec3 LIGHT_OUTLINE_COLOR = vec3(0.859); // #DBDBDB en Linear Burn
const vec3 DARK_OUTLINE_COLOR = vec3(0.651);  // #A6A6A6 en Linear Burn
const vec3 LIGHT_EDGE_GLOW_COLOR = vec3(0.157); // #282828 en Linear Dodge
const vec3 DARK_EDGE_GLOW_COLOR = vec3(0.102);  // #1A1A1A en Linear Dodge
const float EDGE_GLOW_SIGMA = 3.3; // blur 10: Figma recorta la inner shadow y cae antes que blur / 2
const float EDGE_GLOW_OFFSET = 40.0; // offset Y y spread negativo de las inner shadows

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

// Rectángulo redondeado con esquinas continuas. El corner smoothing de Figma
// empieza la curva antes (radio * (1 + smoothing)) y la aplana en la diagonal;
// una superelipse con ese radio y exponente 2 + 2.4 * smoothing conserva el
// mismo punto medio de la esquina que el arco circular original.
float sd_shape(vec2 p, vec2 half_size, float radius) {
    float r = clamp(radius * (1.0 + u_smoothing), 0.0, min(half_size.x, half_size.y));
    float n = 2.0 + 2.4 * u_smoothing;
    vec2 q = abs(p) - half_size + r;
    vec2 c = max(q, 0.0) / max(r, 1e-3);
    float corner = pow(pow(c.x, n) + pow(c.y, n), 1.0 / n) * r;
    return min(max(q.x, q.y), 0.0) + corner - r;
}

float erf_approx(float x) {
    return sign(x) * sqrt(1.0 - exp(-1.2732395 * x * x));
}

// Cobertura de una forma desenfocada con un Gaussiano de desviación `sigma`.
float blurred_coverage(float sd, float sigma) {
    return 0.5 - 0.5 * erf_approx(sd / (max(sigma, 1e-3) * 1.41421356));
}

float hard_coverage(float sd) {
    return clamp(0.5 - sd, 0.0, 1.0);
}

vec2 screen_uv(vec2 px) {
    vec2 uv = clamp(px / u_resolution, 0.0, 1.0);
    uv.y = 1.0 - uv.y;
    return uv;
}

// Perfil "squircle" del bisel: t = 0 en el borde, t = 1 donde empieza la meseta.
float surface_height(float t) {
    float s = 1.0 - t;
    return pow(1.0 - s * s * s * s, 0.25);
}

float surface_slope(float t) {
    float s = 1.0 - t;
    float s3 = s * s * s;
    return s3 * pow(max(1.0 - s3 * s, 1e-4), -0.75);
}

vec3 sample_dispersed(sampler2D tex, vec2 px, vec2 offset) {
    float spread = 0.5 * u_dispersion;
    return vec3(
        texture2D(tex, screen_uv(px + offset * (1.0 - spread))).r,
        texture2D(tex, screen_uv(px + offset)).g,
        texture2D(tex, screen_uv(px + offset * (1.0 + spread))).b
    );
}

// ---------------------------------------------------------------------------
// Capa 1: "Fill + Shadow"
// ---------------------------------------------------------------------------

// Sombra proyectada (clara: 0 8 48 al 25%; oscura: 0 18 48 al 45%). Como en Figma
// ("show behind node" desactivado) solo se ve fuera de la forma.
float layer_shadow(vec2 p, vec2 half_size, float coverage) {
    vec2 offset = vec2(0.0, mix(LIGHT_SHADOW_OFFSET, DARK_SHADOW_OFFSET, u_tint_mode));
    float opacity = mix(LIGHT_SHADOW_OPACITY, DARK_SHADOW_OPACITY, u_tint_mode);
    float sd = sd_shape(p - offset, half_size, u_radius);
    return u_shadow * opacity * blurred_coverage(sd, SHADOW_SIGMA) * (1.0 - coverage);
}

// Contorno: tres drop shadows sin blur en Linear Burn (#DBDBDB claro, #A6A6A6
// oscuro). Una con spread 0.5 alrededor de toda la forma y dos con spread -0.75
// desplazadas ±1.25 px en X, que asoman por los laterales. Cada una se compone
// por separado, así que en los laterales el oscurecimiento se acumula.
vec3 layer_outline(vec3 backdrop, vec2 p, vec2 half_size, float coverage) {
    float ring_all = hard_coverage(sd_shape(p, half_size + 0.5, u_radius + 0.5));
    float ring_left = hard_coverage(sd_shape(p + vec2(1.25, 0.0), half_size - 0.75, u_radius - 0.75));
    float ring_right = hard_coverage(sd_shape(p - vec2(1.25, 0.0), half_size - 0.75, u_radius - 0.75));

    vec3 burn = mix(LIGHT_OUTLINE_COLOR, DARK_OUTLINE_COLOR, u_tint_mode) - 1.0;
    vec3 col = backdrop;
    col = mix(col, max(col + burn, 0.0), max(ring_all - coverage, 0.0));
    col = mix(col, max(col + burn, 0.0), max(ring_left - coverage, 0.0));
    col = mix(col, max(col + burn, 0.0), max(ring_right - coverage, 0.0));
    return col;
}

// Modo de fusión Luminosity (W3C): conserva tono y saturación de `c` con la
// luminancia `lum`.
vec3 set_luminosity(vec3 c, float lum) {
    vec3 weights = vec3(0.3, 0.59, 0.11);
    c += lum - dot(c, weights);
    float l = dot(c, weights);
    float lo = min(c.r, min(c.g, c.b));
    float hi = max(c.r, max(c.g, c.b));
    if (lo < 0.0) {
        c = l + (c - l) * l / max(l - lo, 1e-5);
    }
    if (hi > 1.0) {
        c = l + (c - l) * (1.0 - l) / max(hi - l, 1e-5);
    }
    return c;
}

// Tinte blanco ("Regular"): relleno blanco al 70% en Lighten y gris #BFBFBF al
// 10% en Darken.
vec3 tint_light(vec3 col) {
    col = mix(col, vec3(1.0), 0.7 * u_tint); // lighten(c, blanco) = blanco
    col = mix(col, min(col, LIGHT_TINT_DARKEN_COLOR), 0.1 * u_tint);
    return col;
}

// Tinte negro ("Dark"): dos rellenos #1A1A1A al 50% en Luminosity y uno al 100%
// en Lighten. El fondo conserva su color pero con la luminancia de #1A1A1A.
vec3 tint_dark(vec3 col) {
    float lum = DARK_TINT_COLOR.r;
    col = mix(col, set_luminosity(col, lum), 0.5 * u_tint);
    col = mix(col, set_luminosity(col, lum), 0.5 * u_tint);
    col = mix(col, max(col, DARK_TINT_COLOR), u_tint);
    return col;
}

vec3 layer_tint(vec3 col) {
    return mix(tint_light(col), tint_dark(col), u_tint_mode);
}

// ---------------------------------------------------------------------------
// Capa 2: "Glass Effect"
// ---------------------------------------------------------------------------

vec3 layer_glass(vec2 p, vec2 half_size, float sd) {
    float dist = max(-sd, 0.0);
    float bezel = max(min(u_depth, min(half_size.x, half_size.y)), 1.0);
    float t = clamp(dist / bezel, 0.0, 1.0);

    // Normal 2D hacia fuera a partir del gradiente del SDF.
    vec2 e = vec2(0.5, 0.0);
    vec2 n2 = vec2(
        sd_shape(p + e.xy, half_size, u_radius) - sd_shape(p - e.xy, half_size, u_radius),
        sd_shape(p + e.yx, half_size, u_radius) - sd_shape(p - e.yx, half_size, u_radius)
    );
    float n2_len = length(n2);
    n2 = n2_len > 1e-5 ? n2 / n2_len : vec2(0.0);

    // Refracción: la superficie sube hacia dentro a lo largo de `depth` y el
    // grosor del cristal escala con `refraction`. Un rayo vertical se refracta
    // y se lleva hasta el fondo recorriendo el grosor en ese punto.
    // Las superficies superiores detectan cristal ya compuesto. Repetir toda
    // la óptica produce una lente acuosa; el material apilado reduce de forma
    // continua refracción, frost y tinte, como una familia de vidrio unificada.
    // La máscara usa el mismo blur que el color acumulado. De esta manera las
    // inner shadows del cristal inferior participan en la transición óptica y
    // no dejan una silueta dura cuando el frost superior es alto.
    float glass_below_coverage = texture2D(u_stack_mask, screen_uv(v_px)).r;
    float stack_response = smoothstep(0.0, 1.0, glass_below_coverage);
    float thickness = u_refraction * u_depth * mix(1.0, 0.12, stack_response);
    float h = surface_height(t);
    float slope = surface_slope(t) * thickness / bezel;
    vec3 normal = normalize(vec3(n2 * slope, 1.0));
    vec3 ray = refract(vec3(0.0, 0.0, -1.0), normal, 1.0 / IOR);
    vec2 offset = ray.xy / max(-ray.z, 1e-3) * h * thickness;

    // Frost: la escena desenfocada ya viene hecha; con frost muy bajo se mezcla
    // con la nítida para que no se note la media resolución.
    vec3 stacked = sample_dispersed(u_scene, v_px, offset);
    vec3 blurred = sample_dispersed(u_scene_blur, v_px, offset);
    float frost = clamp(u_frost / 3.0, 0.0, 1.0);

    // `u_scene_blur` is the blurred accumulated stack. Therefore a surface
    // below remains visible through this one, but receives another diffusion
    // pass just like stacked macOS glass.
    vec3 col = mix(stacked, blurred, frost);

    // Phase 8.5 — Adaptive Material Response: how much the sharp and frosted
    // backdrop samples disagree is a free, GPU-resident proxy for "how busy
    // is the backdrop here" — no extra texture fetch, no CPU readback.
    // `u_adaptive_response` (0 by default) scales how much this feeds into
    // the rim/edge highlights below, per 8.7's "busy backgrounds may
    // require stronger separation".
    float local_busyness = clamp(length(stacked - blurred) * 2.0, 0.0, 1.0) * u_adaptive_response;

    // El glass refracta lo que tiene debajo, que ya incluye el tinte de la capa 1.
    // El tinte es uniforme dentro de la forma, así que aplicarlo tras muestrear
    // da el mismo resultado.
    vec3 tinted = layer_tint(col);
    col = mix(tinted, col, 0.78 * stack_response);

    // Phase 8.8 — Clear Material Dimming: darken the transmitted backdrop in
    // proportion to its own local luminance, before highlights are added, so
    // native foreground content the host draws on top of a high-
    // transmission surface stays legible. `u_clear_dimming` is 0 for every
    // shipped preset; a product opts in per style.
    float local_luma = dot(col, vec3(0.299, 0.587, 0.114));
    col *= 1.0 - u_clear_dimming * local_luma * 0.5;

    // Luz especular: franja suave en el borde hacia el que apunta la luz. Con 0°
    // la luz baja desde arriba y el brillo cae en el borde inferior. Calibrado
    // contra el render de Figma: intensidad 0.25 aclara ~7/255 en ese borde.
    float angle = radians(u_light_angle);
    vec2 light = vec2(sin(angle), cos(angle));
    float lit = pow(max(dot(n2, light), 0.0), 1.5);
    float band = 1.0 - smoothstep(0.0, bezel * mix(0.2, 1.0, u_splay), dist);
    col += u_light_intensity * 0.12 * lit * band * mix(1.0, 0.55, stack_response);

    // Filo de 1 px en el borde interior, más fuerte en el lado opuesto a la luz.
    // En el tinte blanco queda saturado; se nota en el negro (medido en Figma).
    // The busyness boost (8.5/8.7) is additive headroom on top of the base
    // response, not a replacement for it, so it stays 0 at `u_adaptive_response = 0`.
    float edge = 1.0 - smoothstep(0.5, 1.5, dist);
    float facing = dot(n2, light);
    float edge_light = 0.45 * pow(max(-facing, 0.0), 3.0) + 0.17 * pow(max(facing, 0.0), 3.0);
    col += u_light_intensity * edge * edge_light * (1.0 + 0.3 * local_busyness);

    // Inner shadows #282828 / #1A1A1A (blur 10, spread -40, offset Y ±40) en
    // Linear Dodge: una franja suave de luz pegada al borde superior y otra al
    // inferior. Figma no agranda el radio con el spread, así que en las esquinas
    // apenas hay luz.
    vec2 glow_half = half_size + EDGE_GLOW_OFFSET;
    vec2 glow_offset = vec2(0.0, EDGE_GLOW_OFFSET);
    float glow_top = 1.0 - blurred_coverage(sd_shape(p - glow_offset, glow_half, u_radius), EDGE_GLOW_SIGMA);
    float glow_bottom = 1.0 - blurred_coverage(sd_shape(p + glow_offset, glow_half, u_radius), EDGE_GLOW_SIGMA);
    vec3 rim_color = mix(LIGHT_EDGE_GLOW_COLOR, DARK_EDGE_GLOW_COLOR, u_tint_mode);
    // Phase 8.5's "backdrop color characteristics" input: real Liquid Glass
    // visibly picks up color from whatever's just outside its edge (an
    // album cover next to the glass, say), not just a fixed light/dark
    // rim. Sample the already-frosted backdrop pushed outward along this
    // pixel's own surface normal `n2` (computed above, points outward —
    // same convention the specular lighting already uses) and scale it
    // down to roughly the fixed constants' own magnitude (~0.1-0.16) so
    // `u_ambient_reflection` blends smoothly between "fixed rim" and
    // "colored by whatever's nearby" instead of suddenly overpowering it.
    vec3 ambient = texture2D(u_scene_blur, screen_uv(v_px + n2 * bezel * 1.5)).rgb * 0.18;
    rim_color = mix(rim_color, ambient, u_ambient_reflection);
    col += rim_color * (glow_top + glow_bottom) * (1.0 + 0.25 * local_busyness);

    // Phase 9.5 — Interaction Illumination: a soft, omnidirectional edge
    // glow while a surface is hovered/pressed/dragged/selected
    // (interaction_energy() in glass.rs maps GlassInteraction to
    // u_interaction). Idle is 0 — a true no-op. Unlike the directional
    // lighting above, this doesn't favor one side toward a light angle —
    // it's the material responding to being touched, not to a light source.
    float interaction_band = 1.0 - smoothstep(0.0, bezel * 0.5, dist);
    col += u_interaction * interaction_band * INTERACTION_GLOW_COLOR * 0.18;

    // Phase 8's remaining infrastructure piece: a final neutral-by-default
    // color grade (saturation/brightness/contrast), applied after highlights
    // so a product can grade the whole composited material in one place
    // instead of fighting individual layer colors.
    float gray = dot(col, vec3(0.299, 0.587, 0.114));
    col = mix(vec3(gray), col, u_saturation);
    col = (col - 0.5) * u_contrast + 0.5 + u_brightness;

    return clamp(col, 0.0, 1.0);
}

void main() {
    vec2 half_size = u_size * 0.5;
    vec2 p = v_px - u_center;
    float sd = sd_shape(p, half_size, u_radius);
    float coverage = hard_coverage(sd);

    float shadow = layer_shadow(p, half_size, coverage);

    // Fuera de la forma y del contorno (la mayor de sus formas es la de spread 0.5):
    // solo sombra, mezclada con alpha.
    float outline_reach = hard_coverage(sd_shape(p, half_size + 0.5, u_radius + 0.5));
    if (outline_reach <= 0.0) {
        if (shadow < 0.002) {
            discard;
        }
        gl_FragColor = vec4(0.0, 0.0, 0.0, shadow);
        return;
    }

    // En el contorno hace falta el fondo para el Linear Burn (fondo + color - 1).
    vec3 backdrop = texture2D(u_scene, screen_uv(v_px)).rgb * (1.0 - shadow);
    vec3 outside = layer_outline(backdrop, p, half_size, coverage);

    if (coverage <= 0.0) {
        gl_FragColor = vec4(outside, 1.0);
        return;
    }

    vec3 glass = layer_glass(p, half_size, sd);
    gl_FragColor = vec4(mix(outside, glass, coverage), 1.0);
}
