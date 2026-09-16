use macroquad::prelude::*;
use spark_glass::glass::*;
use spark_glass::renderer::MacroquadGlassRenderer;
use std::time::Instant;

const BACKGROUNDS: [&[u8]; 4] = [
    include_bytes!("../assets/image1.jpg"),
    include_bytes!("../assets/image2.jpg"),
    include_bytes!("../assets/image3.jpg"),
    include_bytes!("../assets/image4.jpg"),
];

struct Param {
    label: &'static str,
    value: f32,
    min: f32,
    max: f32,
    speed: f32,
}
impl Param {
    const fn new(label: &'static str, min: f32, max: f32, speed: f32) -> Self {
        Self {
            label,
            value: 0.,
            min,
            max,
            speed,
        }
    }
}
#[derive(Clone, Copy)]
struct GlassProfile {
    name: &'static str,
    values: [f32; 9],
    dark: bool,
}
const PARAM_NAMES: [&str; 9] = [
    "refraction",
    "depth",
    "dispersion",
    "frost",
    "light_intensity",
    "light_angle",
    "splay",
    "tint",
    "shadow",
];
const TUNED_PROFILES_PATH: &str = "tests/tuned_profiles.txt";
const PROFILES: [GlassProfile; 3] = [
    GlassProfile {
        name: "Clear",
        values: [2., 30., 0.2, 6., 0.25, 0., 0.2, 0.15, 1.],
        dark: false,
    },
    GlassProfile {
        name: "White tint",
        values: [2., 30., 0.2, 16., 0.25, 0., 0.2, 1., 1.],
        dark: false,
    },
    GlassProfile {
        name: "Black tint",
        values: [2., 30., 0.2, 16., 0.25, 0., 0.2, 1., 1.],
        dark: true,
    },
];

fn window_conf() -> Conf {
    Conf {
        window_title: "Analogic Goose Presents: SPARK GLASS".to_owned(),
        window_width: 1280,
        window_height: 800,
        sample_count: 4,
        ..Default::default()
    }
}
fn load_background(bytes: &[u8]) -> Texture2D {
    let image = image::load_from_memory(bytes)
        .expect("Failed to decode background image")
        .to_rgba8();
    let texture = Texture2D::from_image(&Image {
        width: image.width() as u16,
        height: image.height() as u16,
        bytes: image.into_raw(),
    });
    texture.set_filter(FilterMode::Linear);
    texture
}
fn draw_cover(texture: &Texture2D, width: f32, height: f32) {
    let size = texture.size() * (width / texture.width()).max(height / texture.height());
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

fn debug_hud_surface(_width: f32, height: f32) -> GlassSurface {
    let lines = 16.0;
    let size = vec2(310.0, lines * 20.0 + 12.0);
    let center = vec2(16.0 + size.x * 0.5, height - 16.0 - size.y * 0.5);
    let (material, optics, lighting) = preset(GlassStyle::Thin, true);
    GlassSurface {
        id: u64::MAX,
        geometry: GlassGeometry::RoundedRect {
            center,
            size,
            radius: 14.0,
            smoothing: 0.45,
        },
        material,
        optics,
        lighting,
        interaction: GlassInteraction::Idle,
        style: GlassStyle::Control,
    }
}
fn apply_profile(profile: &GlassProfile, params: &mut [Param], dark: &mut bool) {
    for (param, value) in params.iter_mut().zip(profile.values) {
        param.value = value;
    }
    *dark = profile.dark;
}
fn apply_tuning(scene: &mut GlassScene, params: &[Param], dark: bool) {
    for s in &mut scene.surfaces {
        s.optics.refraction_strength = params[0].value;
        s.optics.depth = params[1].value;
        s.optics.dispersion = params[2].value;
        s.material.frost_radius = params[3].value;
        s.lighting.intensity = params[4].value;
        s.lighting.angle_degrees = params[5].value;
        s.lighting.splay = params[6].value;
        s.material.tint_opacity = params[7].value;
        s.material.dark_tint = dark;
        s.lighting.shadow_strength = params[8].value;
    }
}

/// Config mode: writes the live-tuned state of all 3 profiles to a plain
/// text file, so a value tuned by eye against a reference image doesn't
/// have to be read off the HUD and retyped by hand. Format is deliberately
/// simple (not TOML/JSON) — no new dependency, easy for a human to read,
/// easy for an AI session to parse back out afterward.
fn save_profiles(profiles: &[GlassProfile; 3]) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = String::new();
    out.push_str("# SparkGlass tuned profiles\n");
    out.push_str("# Saved interactively from `cargo run` (press S in the HUD) — not committed to git.\n");
    out.push_str("# To apply: read this file and update src/glass.rs's preset() / main.rs's PROFILES accordingly.\n\n");
    for profile in profiles {
        out.push_str(&format!("[{}]\n", profile.name));
        out.push_str(&format!("dark = {}\n", profile.dark));
        for (name, value) in PARAM_NAMES.iter().zip(profile.values) {
            out.push_str(&format!("{name} = {value:.3}\n"));
        }
        out.push('\n');
    }
    if let Some(parent) = std::path::Path::new(TUNED_PROFILES_PATH).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(TUNED_PROFILES_PATH)?;
    file.write_all(out.as_bytes())
}

fn draw_hud(
    params: &[Param],
    selected: usize,
    background: usize,
    profile: &str,
    dark: bool,
    scene: &GlassScene,
    save_message: Option<&str>,
) {
    // The HUD sits on its own glass panel (debug_hud_surface), which is
    // frosted enough to show whatever's behind it through — so the right
    // text color depends on the *backdrop*, not on `dark`. But `dark` is
    // the best proxy available here without sampling pixels back from the
    // GPU (which the normal render path must never do), and in practice it
    // tracks what the user is looking at closely enough to fix the actual
    // complaint: white-on-white was unreadable against the light "Clear"
    // profile.
    let (text_color, selected_color, hint_color) = if dark {
        (WHITE, YELLOW, LIGHTGRAY)
    } else {
        (BLACK, Color::new(0.55, 0.35, 0.0, 1.0), DARKGRAY)
    };

    let lines = params.len() as f32 + 8. + if save_message.is_some() { 1. } else { 0. };
    let line_h = 20.;
    let (x, y) = (16., screen_height() - 16. - lines * line_h - 12.);
    let mut ty = y + 22.;
    for (i, p) in params.iter().enumerate() {
        draw_text(
            format!(
                "{} {:<12} {:>7.2}",
                if i == selected { ">" } else { " " },
                p.label,
                p.value
            ),
            x + 8.,
            ty,
            20.,
            if i == selected { selected_color } else { text_color },
        );
        ty += line_h;
    }
    for line in [
        format!("Profile {profile}  [P]"),
        format!("Background {}/4  [1-4]", background + 1),
        format!("Tint {}  [T]", if dark { "black" } else { "white" }),
        format!("Quality {:?}  [Q]", scene.quality),
        "Drag panels with the mouse".to_owned(),
        "Arrows: select / adjust  H: hide".to_owned(),
        "D: scene debug   S: save all 3 profiles".to_owned(),
    ] {
        draw_text(&line, x + 8., ty, 18., hint_color);
        ty += line_h;
    }
    if let Some(message) = save_message {
        draw_text(message, x + 8., ty, 18., if dark { GREEN } else { DARKGREEN });
    }
}
fn draw_debug(scene: &GlassScene) {
    for (z, s) in scene.surfaces.iter().enumerate() {
        let g = s.geometry;
        let tl = g.center() - g.size() * 0.5;
        draw_rectangle_lines(tl.x, tl.y, g.size().x, g.size().y, 1., MAGENTA);
        draw_circle(g.center().x, g.center().y, 3., YELLOW);
        draw_text(
            &format!("id:{} z:{} {:?}", s.id, z, s.style),
            tl.x + 8.0,
            tl.y + 20.0,
            18.0,
            YELLOW,
        );
    }
    draw_text(
        &format!(
            "GlassScene: frame {} | shared backdrop | {} surfaces | {:?}",
            scene.frame,
            scene.surfaces.len(),
            scene.quality
        ),
        16.,
        28.,
        20.,
        YELLOW,
    );
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut capture_path = std::env::var_os("SPARK_GLASS_CAPTURE").map(std::path::PathBuf::from);
    let benchmark_frames = std::env::var("SPARK_GLASS_BENCHMARK_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|frames| *frames >= 3);
    let mut frame_samples = Vec::new();
    let mut render_samples = Vec::new();
    let mut renderer = MacroquadGlassRenderer::new(screen_width(), screen_height());
    let mut backgrounds: [Option<Texture2D>; 4] = Default::default();
    let mut background = 0;
    backgrounds[0] = Some(load_background(BACKGROUNDS[0]));
    let (material, optics, lighting) = preset(GlassStyle::Regular, false);
    let (w, h) = (screen_width(), screen_height());
    let mut scene = GlassScene::new(vec![
        GlassSurface {
            id: 1,
            geometry: GlassGeometry::RoundedRect {
                center: vec2(w * 0.5, h * 0.4),
                size: vec2(640., 498.),
                radius: 34.,
                smoothing: 0.6,
            },
            material,
            optics,
            lighting,
            interaction: GlassInteraction::Idle,
            style: GlassStyle::Regular,
        },
        GlassSurface {
            id: 2,
            geometry: GlassGeometry::RoundedRect {
                center: vec2(w * 0.5, h * 0.86),
                size: vec2(380., 88.),
                radius: 44.,
                smoothing: 0.,
            },
            material,
            optics,
            lighting,
            interaction: GlassInteraction::Idle,
            style: GlassStyle::Control,
        },
    ]);
    if std::env::var_os("SPARK_GLASS_TEST_OVERLAP").is_some() {
        scene.surfaces[1].geometry = scene.surfaces[1]
            .geometry
            .with_center(vec2(w * 0.5, h * 0.68));
        scene.surfaces.swap(0, 1);
    }
    let mut params = [
        Param::new("Refraction", 0., 100., 0.5),
        Param::new("Depth", 1., 120., 40.),
        Param::new("Dispersion", 0., 1., 0.5),
        Param::new("Frost", 0., 48., 16.),
        Param::new("Light", 0., 1., 0.5),
        Param::new("Light angle", -180., 180., 90.),
        Param::new("Splay", 0., 1., 0.5),
        Param::new("Tint", 0., 1., 0.5),
        Param::new("Shadow", 0., 2., 1.),
    ];
    let (mut selected, mut show_hud, mut show_debug, mut dark, mut profile, mut drag) =
        (0usize, true, false, false, 0usize, None::<Vec2>);
    // Config mode: a runtime-mutable copy of PROFILES. Every parameter edit
    // below is written back into `profiles[profile]` immediately, so all 3
    // profiles keep their own live-tuned state as you switch between them
    // with P, and S dumps that whole array to disk on demand.
    let mut profiles = PROFILES;
    let mut save_message: Option<String> = None;
    apply_profile(&profiles[profile], &mut params, &mut dark);
    if let Ok(value) = std::env::var("SPARK_GLASS_TEST_FROST")
        && let Ok(frost) = value.parse::<f32>()
    {
        params[3].value = frost.clamp(params[3].min, params[3].max);
    }
    loop {
        let (w, h, dt) = (screen_width(), screen_height(), get_frame_time());
        renderer.resize_if_needed(w, h);
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
        if is_key_pressed(KeyCode::D) {
            show_debug = !show_debug;
        }
        if is_key_pressed(KeyCode::T) {
            dark = !dark;
            profiles[profile].dark = dark;
        }
        if is_key_pressed(KeyCode::P) {
            profile = (profile + 1) % profiles.len();
            apply_profile(&profiles[profile], &mut params, &mut dark);
        }
        if is_key_pressed(KeyCode::S) {
            save_message = Some(match save_profiles(&profiles) {
                Ok(()) => format!("Saved {TUNED_PROFILES_PATH}"),
                Err(e) => format!("Save failed: {e}"),
            });
        }
        if is_key_pressed(KeyCode::Q) {
            scene.quality = match scene.quality {
                GlassQuality::Ultra => GlassQuality::High,
                GlassQuality::High => GlassQuality::Medium,
                GlassQuality::Medium => GlassQuality::Low,
                GlassQuality::Low => GlassQuality::Fallback,
                GlassQuality::Fallback => GlassQuality::Ultra,
            };
        }
        if is_key_pressed(KeyCode::Up) {
            selected = (selected + params.len() - 1) % params.len();
        }
        if is_key_pressed(KeyCode::Down) {
            selected = (selected + 1) % params.len();
        }
        if is_key_down(KeyCode::Right) {
            let p = &mut params[selected];
            p.value = (p.value + p.speed * dt).min(p.max);
            profiles[profile].values[selected] = p.value;
        }
        if is_key_down(KeyCode::Left) {
            let p = &mut params[selected];
            p.value = (p.value - p.speed * dt).max(p.min);
            profiles[profile].values[selected] = p.value;
        }
        let mouse = Vec2::from(mouse_position());
        if is_mouse_button_pressed(MouseButton::Left)
            && let Some(index) = scene.surface_at(mouse)
        {
            let surface = scene.surfaces[index];
            drag = Some(surface.geometry.center() - mouse);
            scene.bring_to_front(index);
        }
        if is_mouse_button_released(MouseButton::Left) {
            drag = None;
        }
        if let (Some(grab), Some(surface)) = (drag, scene.surfaces.last_mut()) {
            surface.geometry = surface.geometry.with_center(mouse + grab);
            surface.interaction = GlassInteraction::Dragged;
        } else {
            for s in &mut scene.surfaces {
                s.interaction = GlassInteraction::Idle;
            }
        }
        apply_tuning(&mut scene, &params, dark);
        scene.frame += 1;
        if show_hud {
            scene.surfaces.push(debug_hud_surface(w, h));
        }
        let render_start = Instant::now();
        renderer.begin_backdrop(|| {
            if let Some(texture) = &backgrounds[background] {
                draw_cover(texture, w, h);
            }
        });
        renderer.render(&scene, w, h);
        if show_hud {
            scene.surfaces.pop();
        }
        if scene.frame > 2 {
            frame_samples.push(dt * 1000.0);
            render_samples.push(render_start.elapsed().as_secs_f32() * 1000.0);
        }
        if show_debug {
            draw_debug(&scene);
        }
        if show_hud {
            draw_hud(
                &params,
                selected,
                background,
                profiles[profile].name,
                dark,
                &scene,
                save_message.as_deref(),
            );
        }
        if scene.frame >= 5
            && let Some(path) = capture_path.take()
        {
            get_screen_data().export_png(&path.to_string_lossy());
            break;
        }
        if let Some(frame_count) = benchmark_frames
            && scene.frame >= frame_count
        {
            let stats = |samples: &mut Vec<f32>| {
                samples.sort_by(f32::total_cmp);
                let average = samples.iter().sum::<f32>() / samples.len() as f32;
                let p95 = samples[((samples.len() - 1) * 95) / 100];
                let max = *samples.last().unwrap_or(&0.0);
                (average, p95, max)
            };
            let (frame_avg, frame_p95, frame_max) = stats(&mut frame_samples);
            let (render_avg, render_p95, render_max) = stats(&mut render_samples);
            println!(
                "BENCHMARK frames={} frame_ms(avg/p95/max)={:.3}/{:.3}/{:.3} fps={:.2} render_cpu_ms(avg/p95/max)={:.3}/{:.3}/{:.3}",
                frame_count,
                frame_avg,
                frame_p95,
                frame_max,
                1000.0 / frame_avg.max(0.001),
                render_avg,
                render_p95,
                render_max,
            );
            break;
        }
        next_frame().await;
    }
}
