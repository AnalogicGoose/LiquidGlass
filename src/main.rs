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

/// Number of static hint lines `draw_hud` always draws below the parameter
/// list (Profile/Background/Tint/Quality/drag hint/arrows hint/keys hint).
const HUD_HINT_LINES: usize = 7;
const HUD_PARAM_FONT: u16 = 20;
const HUD_HINT_FONT: u16 = 18;
const HUD_LINE_H: f32 = 20.0;
const HUD_TEXT_X: f32 = 8.0;
/// Distance from the panel's top edge to the first line's text baseline.
const HUD_FIRST_BASELINE: f32 = 22.0;
/// Distance from the last line's text baseline down to the panel's bottom
/// edge — generous enough to keep descenders inside the panel even though
/// macroquad's font metrics don't line up exactly with `HUD_LINE_H`.
const HUD_BOTTOM_PAD: f32 = 36.0;

/// The HUD panel's text content — the single source of truth for both
/// sizing the glass panel behind it (`hud_size`, used by `debug_hud_surface`)
/// and drawing the text on top of it (`draw_hud`), so the two can never
/// disagree about how many lines there are, how wide they are, or what they
/// say. A save/reset confirmation replaces the last hint line in place
/// rather than appending an 8th, so the line count here never changes for
/// that.
fn hud_texts(
    params: &[Param],
    selected: usize,
    profile: &str,
    background: usize,
    dark: bool,
    quality: GlassQuality,
    save_message: Option<&str>,
) -> (Vec<String>, [String; HUD_HINT_LINES]) {
    let param_lines = params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "{} {:<12} {:>7.2}",
                if i == selected { ">" } else { " " },
                p.label,
                p.value
            )
        })
        .collect();
    let hint_lines = [
        format!("Profile {profile}  [P]  R: reset it"),
        format!("Background {}/4  [1-4]", background + 1),
        format!("Tint {}  [T]", if dark { "black" } else { "white" }),
        format!("Quality {quality:?}  [Q]"),
        "Drag panels with the mouse".to_owned(),
        "Arrows: select / adjust  H: hide".to_owned(),
        save_message
            .map(str::to_owned)
            .unwrap_or_else(|| "D: scene debug   S: save all 3 profiles".to_owned()),
    ];
    (param_lines, hint_lines)
}

/// The panel size for exactly this HUD content: wide enough for its longest
/// line (measured, not guessed — profile names and enum labels vary in
/// width) and tall enough for every line.
fn hud_size(param_lines: &[String], hint_lines: &[String]) -> Vec2 {
    let mut max_w = 0.0f32;
    for line in param_lines {
        max_w = max_w.max(measure_text(line, None, HUD_PARAM_FONT, 1.0).width);
    }
    for line in hint_lines {
        max_w = max_w.max(measure_text(line, None, HUD_HINT_FONT, 1.0).width);
    }
    let lines = (param_lines.len() + hint_lines.len()) as f32;
    vec2(
        max_w + HUD_TEXT_X * 2.0,
        HUD_FIRST_BASELINE + (lines - 1.0) * HUD_LINE_H + HUD_BOTTOM_PAD,
    )
}

fn debug_hud_surface(height: f32, param_lines: &[String], hint_lines: &[String]) -> GlassSurface {
    let size = hud_size(param_lines, hint_lines);
    let center = vec2(16.0 + size.x * 0.5, height - 16.0 - size.y * 0.5);
    // Always a strongly-tinted, near-opaque dark panel — not the thin/
    // adaptive glass used elsewhere. The HUD sits over whatever backdrop or
    // profile the user is currently looking at, so its own readability
    // can't depend on either; a fixed high-opacity dark panel with fixed
    // light text is the only combination that's reliably legible.
    let (mut material, optics, lighting) = preset(GlassStyle::Thin, true);
    material.dark_tint = true;
    material.tint_opacity = 0.92;
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
    param_lines: &[String],
    hint_lines: &[String],
    selected: usize,
    save_message_active: bool,
    height: f32,
) {
    // The panel itself (debug_hud_surface) is now always a fixed dark,
    // near-opaque tint, so the text on top of it can be a fixed light
    // palette too — no more guessing at contrast against whatever backdrop
    // or profile happens to be showing through.
    let (text_color, selected_color, hint_color) = (WHITE, YELLOW, LIGHTGRAY);

    let size = hud_size(param_lines, hint_lines);
    let x = 16.;
    let panel_top = height - 16. - size.y;
    let mut ty = panel_top + HUD_FIRST_BASELINE;
    for (i, line) in param_lines.iter().enumerate() {
        draw_text(
            line,
            x + HUD_TEXT_X,
            ty,
            HUD_PARAM_FONT as f32,
            if i == selected { selected_color } else { text_color },
        );
        ty += HUD_LINE_H;
    }
    for (i, line) in hint_lines.iter().enumerate() {
        let is_save_message = save_message_active && i == hint_lines.len() - 1;
        let color = if is_save_message { GREEN } else { hint_color };
        draw_text(line, x + HUD_TEXT_X, ty, HUD_HINT_FONT as f32, color);
        ty += HUD_LINE_H;
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
    let mut save_message: Option<(String, Instant)> = None;
    const SAVE_MESSAGE_LIFETIME: std::time::Duration = std::time::Duration::from_secs(3);
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
            let message = match save_profiles(&profiles) {
                Ok(()) => format!("Saved {TUNED_PROFILES_PATH}"),
                Err(e) => format!("Save failed: {e}"),
            };
            save_message = Some((message, Instant::now()));
        }
        if is_key_pressed(KeyCode::R) {
            // Reset *only* the current profile back to its shipped default
            // — since edits now persist across profile switches (that's
            // the point of config mode), it's easy to drift a profile away
            // from its original values while just exploring, with no way
            // back short of restarting. This is that way back.
            profiles[profile] = PROFILES[profile];
            apply_profile(&profiles[profile], &mut params, &mut dark);
            save_message = Some((format!("Reset {} to defaults", profiles[profile].name), Instant::now()));
        }
        if let Some((_, saved_at)) = &save_message
            && saved_at.elapsed() > SAVE_MESSAGE_LIFETIME
        {
            save_message = None;
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
        let (param_lines, hint_lines) = hud_texts(
            &params,
            selected,
            profiles[profile].name,
            background,
            dark,
            scene.quality,
            save_message.as_ref().map(|(message, _)| message.as_str()),
        );
        if show_hud {
            scene
                .surfaces
                .push(debug_hud_surface(h, &param_lines, &hint_lines));
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
                &param_lines,
                &hint_lines,
                selected,
                save_message.is_some(),
                h,
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
