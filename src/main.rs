use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::prelude::*;

const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;

varying vec2 v_px;
varying vec2 v_uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    // Con la cámara por defecto, position ya está en píxeles de pantalla (y hacia abajo).
    v_px = position.xy;
    v_uv = texcoord;
    gl_Position = Projection * Model * vec4(position, 1.0);
}
"#;

const GLASS_SHADER: &str = include_str!("glass.frag");
const BLUR_SHADER: &str = include_str!("blur.frag");

const BACKGROUNDS: [&[u8]; 4] = [
    include_bytes!("../assets/image1.jpg"),
    include_bytes!("../assets/image2.jpg"),
    include_bytes!("../assets/image3.jpg"),
    include_bytes!("../assets/image4.jpg"),
];

/// Margen alrededor del panel que también se rasteriza, para que quepa la sombra
/// (offset de hasta 18 + ~3 sigma de un blur de 48).
const SHADOW_MARGIN: f32 = 96.0;

struct GlassPanel {
    center: Vec2,
    size: Vec2,
    radius: f32,
    /// Corner smoothing de Figma: 0 = esquina circular, 0.6 = estilo iOS.
    smoothing: f32,
}

impl GlassPanel {
    fn contains(&self, point: Vec2) -> bool {
        let half = self.size * 0.5;
        let r = self.radius.min(half.x).min(half.y);
        let q = (point - self.center).abs() - half + vec2(r, r);
        q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r <= 0.0
    }
}

/// Parámetro del material que se puede ajustar en vivo con el teclado.
struct Param {
    label: &'static str,
    uniform: &'static str,
    value: f32,
    min: f32,
    max: f32,
    /// Unidades por segundo al mantener pulsada la flecha.
    speed: f32,
}

impl Param {
    const fn new(
        label: &'static str,
        uniform: &'static str,
        value: f32,
        min: f32,
        max: f32,
        speed: f32,
    ) -> Self {
        Self {
            label,
            uniform,
            value,
            min,
            max,
            speed,
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Liquid Glass PoC".to_owned(),
        window_width: 1280,
        window_height: 800,
        sample_count: 4,
        ..Default::default()
    }
}

fn load_background(bytes: &[u8]) -> Texture2D {
    let img = image::load_from_memory(bytes)
        .expect("No se pudo decodificar la imagen de fondo")
        .to_rgba8();

    let texture = Texture2D::from_image(&Image {
        width: img.width() as u16,
        height: img.height() as u16,
        bytes: img.into_raw(),
    });
    texture.set_filter(FilterMode::Linear);
    texture
}

fn new_target(width: u32, height: u32) -> RenderTarget {
    let target = render_target(width.max(1), height.max(1));
    target.texture.set_filter(FilterMode::Linear);
    target
}

/// Render targets de la escena: nítida a resolución completa y dos a media
/// resolución para hacer el Gaussiano separable (horizontal en `blur_tmp`,
/// vertical de vuelta en `blur`).
struct SceneTargets {
    sharp: RenderTarget,
    blur: RenderTarget,
    blur_tmp: RenderTarget,
}

impl SceneTargets {
    fn new(width: f32, height: f32) -> Self {
        let (w, h) = (width as u32, height as u32);
        Self {
            sharp: new_target(w, h),
            blur: new_target(w / 2, h / 2),
            blur_tmp: new_target(w / 2, h / 2),
        }
    }
}

/// Activa una cámara que dibuja en `target` con coordenadas en píxeles del target.
fn set_target_camera(target: &RenderTarget) {
    let size = target.texture.size();
    let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
    camera.render_target = Some(target.clone());
    set_camera(&camera);
}

/// Copia `source` sobre todo el target activo. Las texturas de render target
/// quedan invertidas en Y, de ahí el `flip_y`.
fn blit(source: &Texture2D, dest_size: Vec2) {
    draw_texture_ex(
        source,
        0.0,
        0.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(dest_size),
            flip_y: true,
            ..Default::default()
        },
    );
}

/// Desenfoca `targets.sharp` y deja el resultado en `targets.blur`.
fn blur_scene(targets: &SceneTargets, material: &Material, sigma_px: f32) {
    let half = targets.blur.texture.size();

    // Reducir a la mitad con filtrado lineal promedia bloques de 2x2.
    set_target_camera(&targets.blur);
    blit(&targets.sharp.texture, half);

    // El sigma llega en píxeles de pantalla; aquí trabajamos a media resolución.
    material.set_uniform("u_texel", 1.0 / half);
    material.set_uniform("u_sigma", sigma_px * 0.5);

    for (source, dest, direction) in [
        (&targets.blur, &targets.blur_tmp, vec2(1.0, 0.0)),
        (&targets.blur_tmp, &targets.blur, vec2(0.0, 1.0)),
    ] {
        set_target_camera(dest);
        material.set_uniform("u_direction", direction);
        gl_use_material(material);
        blit(&source.texture, half);
        gl_use_default_material();
    }
}

/// Dibuja la textura cubriendo toda la pantalla sin deformarla (como `object-fit: cover`).
fn draw_cover(texture: &Texture2D, width: f32, height: f32) {
    let scale = (width / texture.width()).max(height / texture.height());
    let size = texture.size() * scale;

    draw_texture_ex(
        texture,
        (width - size.x) * 0.5,
        (height - size.y) * 0.5,
        WHITE,
        DrawTextureParams {
            dest_size: Some(size),
            ..Default::default()
        },
    );
}

fn draw_hud(params: &[Param], selected: usize, background: usize, dark_tint: bool) {
    let lines = params.len() as f32 + 4.0;
    let line_h = 20.0;
    let (x, y) = (16.0, screen_height() - 16.0 - lines * line_h - 12.0);

    draw_rectangle(
        x,
        y,
        290.0,
        lines * line_h + 12.0,
        Color::new(0.0, 0.0, 0.0, 0.45),
    );

    let mut ty = y + 22.0;
    for (i, p) in params.iter().enumerate() {
        let color = if i == selected { YELLOW } else { WHITE };
        let marker = if i == selected { ">" } else { " " };
        draw_text(
            format!("{marker} {:<12} {:>7.2}", p.label, p.value),
            x + 8.0,
            ty,
            20.0,
            color,
        );
        ty += line_h;
    }

    let help = [
        format!("Fondo {}/4  [1-4]", background + 1),
        format!("Tinte {}  [T]", if dark_tint { "negro" } else { "blanco" }),
        "Arrastra los paneles con el raton".to_owned(),
        "Flechas: elegir / ajustar  H: ocultar".to_owned(),
    ];
    for line in help {
        draw_text(&line, x + 8.0, ty, 18.0, LIGHTGRAY);
        ty += line_h;
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let glass_material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: GLASS_SHADER,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("u_resolution", UniformType::Float2),
                UniformDesc::new("u_center", UniformType::Float2),
                UniformDesc::new("u_size", UniformType::Float2),
                UniformDesc::new("u_radius", UniformType::Float1),
                UniformDesc::new("u_smoothing", UniformType::Float1),
                UniformDesc::new("u_refraction", UniformType::Float1),
                UniformDesc::new("u_depth", UniformType::Float1),
                UniformDesc::new("u_dispersion", UniformType::Float1),
                UniformDesc::new("u_frost", UniformType::Float1),
                UniformDesc::new("u_light_intensity", UniformType::Float1),
                UniformDesc::new("u_light_angle", UniformType::Float1),
                UniformDesc::new("u_splay", UniformType::Float1),
                UniformDesc::new("u_tint_mode", UniformType::Float1),
                UniformDesc::new("u_tint", UniformType::Float1),
                UniformDesc::new("u_shadow", UniformType::Float1),
            ],
            textures: vec!["u_scene".to_owned(), "u_scene_blur".to_owned()],
        },
    )
    .unwrap();

    let blur_material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: BLUR_SHADER,
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("u_texel", UniformType::Float2),
                UniformDesc::new("u_direction", UniformType::Float2),
                UniformDesc::new("u_sigma", UniformType::Float1),
            ],
            ..Default::default()
        },
    )
    .unwrap();

    // Decodificar JPEGs grandes es lento en debug: se cargan bajo demanda.
    let mut backgrounds: [Option<Texture2D>; 4] = Default::default();
    let mut background = 0;
    backgrounds[background] = Some(load_background(BACKGROUNDS[background]));

    let mut scene = SceneTargets::new(screen_width(), screen_height());

    // Valores por defecto sacados de Figma: "Liquid Glass - Regular - Large".
    let mut params = [
        // Capa "Glass Effect" (efecto GLASS)
        Param::new("Refraccion", "u_refraction", 0.7, 0.0, 100.0, 0.5),
        Param::new("Profundidad", "u_depth", 30.0, 1.0, 120.0, 40.0),
        Param::new("Dispersion", "u_dispersion", 0.2, 0.0, 1.0, 0.5),
        Param::new("Frost", "u_frost", 16.0, 0.0, 48.0, 16.0),
        Param::new("Luz", "u_light_intensity", 0.25, 0.0, 1.0, 0.5),
        Param::new("Angulo luz", "u_light_angle", 0.0, -180.0, 180.0, 90.0),
        Param::new("Splay", "u_splay", 0.2, 0.0, 1.0, 0.5),
        // Capa "Fill + Shadow": 1 = opacidades de Figma de la variante activa
        Param::new("Tinte", "u_tint", 1.0, 0.0, 1.0, 0.5),
        Param::new("Sombra", "u_shadow", 1.0, 0.0, 2.0, 1.0),
    ];
    let mut selected = 0;
    let mut show_hud = true;
    // Variante del tinte: blanca ("Liquid Glass - Regular") u oscura ("Liquid Glass - Dark").
    let mut dark_tint = false;

    let (w, h) = (screen_width(), screen_height());
    let mut panels = vec![
        // Mismo tamaño y esquinas que el frame de Figma.
        GlassPanel {
            center: vec2(w * 0.5, h * 0.4),
            size: vec2(640.0, 498.0),
            radius: 34.0,
            smoothing: 0.6,
        },
        GlassPanel {
            center: vec2(w * 0.5, h * 0.86),
            size: vec2(380.0, 88.0),
            radius: 44.0,
            // Cápsula: con radio = media altura el smoothing la volvería más cuadrada.
            smoothing: 0.0,
        },
    ];
    let mut drag: Option<Vec2> = None;

    loop {
        let (w, h) = (screen_width(), screen_height());
        let dt = get_frame_time();

        // ---- Entrada ----
        for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
            .into_iter()
            .enumerate()
        {
            if is_key_pressed(key) {
                background = i;
                if backgrounds[i].is_none() {
                    backgrounds[i] = Some(load_background(BACKGROUNDS[i]));
                }
            }
        }
        if is_key_pressed(KeyCode::H) {
            show_hud = !show_hud;
        }
        if is_key_pressed(KeyCode::T) {
            dark_tint = !dark_tint;
        }
        if is_key_pressed(KeyCode::Up) {
            selected = (selected + params.len() - 1) % params.len();
        }
        if is_key_pressed(KeyCode::Down) {
            selected = (selected + 1) % params.len();
        }
        let param = &mut params[selected];
        if is_key_down(KeyCode::Right) {
            param.value = (param.value + param.speed * dt).min(param.max);
        }
        if is_key_down(KeyCode::Left) {
            param.value = (param.value - param.speed * dt).max(param.min);
        }

        // El panel que se agarra pasa al final del Vec para dibujarse encima.
        let mouse = Vec2::from(mouse_position());
        if is_mouse_button_pressed(MouseButton::Left)
            && let Some(i) = panels.iter().rposition(|panel| panel.contains(mouse))
        {
            let panel = panels.remove(i);
            drag = Some(panel.center - mouse);
            panels.push(panel);
        }
        if is_mouse_button_released(MouseButton::Left) {
            drag = None;
        }
        if let (Some(grab), Some(panel)) = (drag, panels.last_mut()) {
            panel.center = mouse + grab;
        }

        // ---- 1. Escena (lo que hay detrás del cristal) en render targets ----
        if scene.sharp.texture.size() != vec2(w, h) {
            scene = SceneTargets::new(w, h);
        }

        set_target_camera(&scene.sharp);
        clear_background(BLACK);
        if let Some(texture) = &backgrounds[background] {
            draw_cover(texture, w, h);
        }

        // El frost de Figma es un radio de blur: sigma = frost / 2.
        let frost = params
            .iter()
            .find(|p| p.uniform == "u_frost")
            .map_or(0.0, |p| p.value);
        blur_scene(&scene, &blur_material, frost * 0.5);

        // ---- 2. Pantalla: escena + paneles de cristal ----
        set_default_camera();
        clear_background(BLACK);
        blit(&scene.sharp.texture, vec2(w, h));

        glass_material.set_texture("u_scene", scene.sharp.texture.clone());
        glass_material.set_texture("u_scene_blur", scene.blur.texture.clone());
        glass_material.set_uniform("u_resolution", vec2(w, h));
        glass_material.set_uniform("u_tint_mode", if dark_tint { 1.0f32 } else { 0.0f32 });
        for p in &params {
            glass_material.set_uniform(p.uniform, p.value);
        }

        for panel in &panels {
            glass_material.set_uniform("u_center", panel.center);
            glass_material.set_uniform("u_size", panel.size);
            glass_material.set_uniform("u_radius", panel.radius);
            glass_material.set_uniform("u_smoothing", panel.smoothing);

            // macroquad guarda los uniforms por draw call; cambiar de material entre
            // paneles evita que se agrupen en un solo batch con los valores del último.
            gl_use_material(&glass_material);
            let top_left = panel.center - panel.size * 0.5 - SHADOW_MARGIN;
            let quad = panel.size + SHADOW_MARGIN * 2.0;
            draw_rectangle(top_left.x, top_left.y, quad.x, quad.y, WHITE);
            gl_use_default_material();
        }

        if show_hud {
            draw_hud(&params, selected, background, dark_tint);
        }

        next_frame().await;
    }
}
