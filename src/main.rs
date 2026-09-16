use macroquad::hash;
use macroquad::prelude::*;
use macroquad::ui::{root_ui, widgets};
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
}
impl Param {
    const fn new(label: &'static str, min: f32, max: f32) -> Self {
        Self {
            label,
            value: 0.,
            min,
            max,
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

/// Demo scenes navigable via `[`/`]` or the config panel's Demo buttons.
/// Only `Reference` is driven by the config-mode sliders/profiles below —
/// the others are fixed comparison scenes to look at, not tune, mirroring
/// `examples/visual_regression.rs`'s named scenes so both tools agree on
/// what these look like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemoScene {
    Reference,
    PanelOnly,
    PillOnly,
    DarkTintPanel,
    OverlappingPanels,
    StyleGallery,
}
impl DemoScene {
    const ALL: [DemoScene; 6] = [
        DemoScene::Reference,
        DemoScene::PanelOnly,
        DemoScene::PillOnly,
        DemoScene::DarkTintPanel,
        DemoScene::OverlappingPanels,
        DemoScene::StyleGallery,
    ];
    fn label(self) -> &'static str {
        match self {
            DemoScene::Reference => "Reference (tunable)",
            DemoScene::PanelOnly => "Panel only",
            DemoScene::PillOnly => "Pill only",
            DemoScene::DarkTintPanel => "Dark tint panel",
            DemoScene::OverlappingPanels => "Overlapping panels",
            DemoScene::StyleGallery => "Style gallery",
        }
    }
    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|s| *s == self)
            .expect("self is always a member of ALL")
    }
    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }
    fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

fn panel(id: u64, center: Vec2, dark: bool) -> GlassSurface {
    let (material, optics, lighting) = preset(GlassStyle::Regular, dark);
    GlassSurface {
        id,
        geometry: GlassGeometry::RoundedRect {
            center,
            size: vec2(640., 498.),
            radius: 34.,
            smoothing: 0.6,
        },
        material,
        optics,
        lighting,
        interaction: GlassInteraction::Idle,
        style: GlassStyle::Regular,
    }
}
fn pill(id: u64, center: Vec2) -> GlassSurface {
    let (material, optics, lighting) = preset(GlassStyle::Control, false);
    GlassSurface {
        id,
        geometry: GlassGeometry::RoundedRect {
            center,
            size: vec2(380., 88.),
            radius: 44.,
            smoothing: 0.,
        },
        material,
        optics,
        lighting,
        interaction: GlassInteraction::Idle,
        style: GlassStyle::Control,
    }
}
fn style_swatch(id: u64, center: Vec2, style: GlassStyle) -> GlassSurface {
    let (material, optics, lighting) = preset(style, false);
    GlassSurface {
        id,
        geometry: GlassGeometry::RoundedRect {
            center,
            size: vec2(220., 160.),
            radius: 24.,
            smoothing: 0.5,
        },
        material,
        optics,
        lighting,
        interaction: GlassInteraction::Idle,
        style,
    }
}

/// Builds the fixed surface list for a demo scene. `Reference`'s surfaces
/// are the ones `apply_tuning` drives from the config-mode sliders/profiles;
/// every other scene is a fixed comparison rendering.
fn build_demo_surfaces(demo: DemoScene, w: f32, h: f32) -> Vec<GlassSurface> {
    match demo {
        DemoScene::Reference => vec![panel(1, vec2(w * 0.5, h * 0.4), false), pill(2, vec2(w * 0.5, h * 0.86))],
        DemoScene::PanelOnly => vec![panel(1, vec2(w * 0.5, h * 0.5), false)],
        DemoScene::PillOnly => vec![pill(1, vec2(w * 0.5, h * 0.5))],
        DemoScene::DarkTintPanel => vec![panel(1, vec2(w * 0.5, h * 0.5), true)],
        DemoScene::OverlappingPanels => vec![
            panel(1, vec2(w * 0.42, h * 0.5), false),
            panel(2, vec2(w * 0.58, h * 0.5), false),
        ],
        DemoScene::StyleGallery => {
            let spacing = 250.0;
            let start_x = w * 0.5 - spacing * 2.0;
            [
                GlassStyle::Thin,
                GlassStyle::Control,
                GlassStyle::Regular,
                GlassStyle::Navigation,
                GlassStyle::Prominent,
            ]
            .into_iter()
            .enumerate()
            .map(|(i, style)| style_swatch((i + 1) as u64, vec2(start_x + spacing * i as f32, h * 0.5), style))
            .collect()
        }
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
    out.push_str("# Saved interactively from `cargo run` (press S / the Save button) — not committed to git.\n");
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

fn ensure_background_loaded(i: usize, background: &mut usize, backgrounds: &mut [Option<Texture2D>; 4]) {
    *background = i;
    if backgrounds[i].is_none() {
        backgrounds[i] = Some(load_background(BACKGROUNDS[i]));
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
    // Not compared against anything by scripts/verify_backends.sh (that
    // script's byte-identical check uses examples/sandbox.rs as its
    // baseline, not this binary) — this is a manual/informational
    // reference capture only. Note it will NOT include the config panel:
    // macroquad::ui defers its own draw to `next_frame()`'s internal
    // end_frame() (see macroquad's Stage::draw), which runs after this
    // frame's `get_screen_data()` call below, not before it. That's a
    // property of macroquad's UI, not a bug here — confirmed by comparing
    // against a real OS-level screenshot of the live window, which does
    // show the panel.
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

    let (w, h) = (screen_width(), screen_height());
    let mut demo_scene = DemoScene::Reference;
    let mut previous_demo_scene = demo_scene;
    let mut scene = GlassScene::new(build_demo_surfaces(demo_scene, w, h));
    if std::env::var_os("SPARK_GLASS_TEST_OVERLAP").is_some() {
        scene.surfaces[1].geometry = scene.surfaces[1]
            .geometry
            .with_center(vec2(w * 0.5, h * 0.68));
        scene.surfaces.swap(0, 1);
    }
    let mut params = [
        Param::new("Refraction", 0., 100.),
        Param::new("Depth", 1., 120.),
        Param::new("Dispersion", 0., 1.),
        Param::new("Frost", 0., 48.),
        Param::new("Light", 0., 1.),
        Param::new("Light angle", -180., 180.),
        Param::new("Splay", 0., 1.),
        Param::new("Tint", 0., 1.),
        Param::new("Shadow", 0., 2.),
    ];
    let (mut show_hud, mut show_debug, mut dark, mut profile, mut drag) =
        (true, false, false, 0usize, None::<Vec2>);
    // Config mode: a runtime-mutable copy of PROFILES. Every slider drag
    // below is written back into `profiles[profile]` immediately, so all 3
    // profiles keep their own live-tuned state as you switch between them,
    // and Save dumps that whole array to disk on demand.
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
        let _ = dt;
        renderer.resize_if_needed(w, h);

        // Keyboard shortcuts kept alongside the config panel's buttons/
        // sliders below — either one reaches the same state.
        for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
            .into_iter()
            .enumerate()
        {
            if is_key_pressed(key) {
                ensure_background_loaded(i, &mut background, &mut backgrounds);
            }
        }
        if is_key_pressed(KeyCode::H) {
            show_hud = !show_hud;
        }
        if is_key_pressed(KeyCode::D) {
            show_debug = !show_debug;
        }
        if is_key_pressed(KeyCode::LeftBracket) {
            demo_scene = demo_scene.prev();
        }
        if is_key_pressed(KeyCode::RightBracket) {
            demo_scene = demo_scene.next();
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

        // Config panel: macroquad's own immediate-mode UI (sliders with a
        // built-in numeric input box, buttons) instead of hand-drawn text
        // + arrow-key adjustment. Every widget call both reads input AND
        // queues its own draw for this frame — no separate "draw the HUD"
        // step needed the way the old text HUD required.
        if show_hud {
            let panel_size = vec2(340.0, 620.0);
            widgets::Window::new(hash!(), vec2(16.0, 16.0), panel_size)
                .label("SparkGlass Config")
                .titlebar(true)
                .movable(true)
                .ui(&mut root_ui(), |ui| {
                    widgets::Label::new(format!("Demo: {}", demo_scene.label())).ui(ui);
                    if widgets::Button::new("< Prev").ui(ui) {
                        demo_scene = demo_scene.prev();
                    }
                    ui.same_line(0.0);
                    if widgets::Button::new("Next >").ui(ui) {
                        demo_scene = demo_scene.next();
                    }
                    ui.separator();

                    if demo_scene == DemoScene::Reference {
                        widgets::Label::new(format!("Profile: {}", profiles[profile].name)).ui(ui);
                        if widgets::Button::new("< Prev").ui(ui) {
                            profile = (profile + profiles.len() - 1) % profiles.len();
                            apply_profile(&profiles[profile], &mut params, &mut dark);
                        }
                        ui.same_line(0.0);
                        if widgets::Button::new("Next >").ui(ui) {
                            profile = (profile + 1) % profiles.len();
                            apply_profile(&profiles[profile], &mut params, &mut dark);
                        }
                        if widgets::Button::new("Reset profile [R]").ui(ui) {
                            profiles[profile] = PROFILES[profile];
                            apply_profile(&profiles[profile], &mut params, &mut dark);
                            save_message =
                                Some((format!("Reset {} to defaults", profiles[profile].name), Instant::now()));
                        }
                        ui.separator();

                        for (i, param) in params.iter_mut().enumerate() {
                            widgets::Slider::new(hash!("param", i), param.min..param.max)
                                .label(param.label)
                                .ui(ui, &mut param.value);
                            profiles[profile].values[i] = param.value;
                        }
                        let mut dark_tint = dark;
                        widgets::Checkbox::new(hash!("dark_tint"))
                            .label("Dark tint  [T]")
                            .ui(ui, &mut dark_tint);
                        if dark_tint != dark {
                            dark = dark_tint;
                            profiles[profile].dark = dark;
                        }
                        if widgets::Button::new("Save all 3 profiles [S]").ui(ui) {
                            let message = match save_profiles(&profiles) {
                                Ok(()) => format!("Saved {TUNED_PROFILES_PATH}"),
                                Err(e) => format!("Save failed: {e}"),
                            };
                            save_message = Some((message, Instant::now()));
                        }
                    } else {
                        widgets::Label::new("(sliders apply to the Reference demo only)").ui(ui);
                    }
                    ui.separator();

                    widgets::Label::new(format!("Background {}/4", background + 1)).ui(ui);
                    for i in 0..4 {
                        if widgets::Button::new(format!("{}", i + 1)).ui(ui) {
                            ensure_background_loaded(i, &mut background, &mut backgrounds);
                        }
                        if i < 3 {
                            ui.same_line(0.0);
                        }
                    }

                    widgets::Label::new(format!("Quality: {:?}", scene.quality)).ui(ui);
                    if widgets::Button::new("Cycle quality [Q]").ui(ui) {
                        scene.quality = match scene.quality {
                            GlassQuality::Ultra => GlassQuality::High,
                            GlassQuality::High => GlassQuality::Medium,
                            GlassQuality::Medium => GlassQuality::Low,
                            GlassQuality::Low => GlassQuality::Fallback,
                            GlassQuality::Fallback => GlassQuality::Ultra,
                        };
                    }
                    ui.separator();
                    widgets::Label::new("Drag panels with the mouse.  [ / ]: demo  H: hide  D: debug").ui(ui);
                    if let Some((message, _)) = &save_message {
                        widgets::Label::new(message.as_str()).ui(ui);
                    }
                });
        }

        if demo_scene != previous_demo_scene {
            scene.surfaces = build_demo_surfaces(demo_scene, w, h);
            previous_demo_scene = demo_scene;
            drag = None;
        }

        let mouse = Vec2::from(mouse_position());
        // Don't let a click on the config panel also grab whatever glass
        // surface happens to be underneath it.
        let ui_has_mouse = root_ui().is_mouse_over(mouse);
        if !ui_has_mouse
            && is_mouse_button_pressed(MouseButton::Left)
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
        if demo_scene == DemoScene::Reference {
            apply_tuning(&mut scene, &params, dark);
        }
        scene.frame += 1;
        let render_start = Instant::now();
        renderer.begin_backdrop(|| {
            if let Some(texture) = &backgrounds[background] {
                draw_cover(texture, w, h);
            }
        });
        renderer.render(&scene, w, h);
        if scene.frame > 2 {
            frame_samples.push(get_frame_time() * 1000.0);
            render_samples.push(render_start.elapsed().as_secs_f32() * 1000.0);
        }
        if show_debug {
            draw_debug(&scene);
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
