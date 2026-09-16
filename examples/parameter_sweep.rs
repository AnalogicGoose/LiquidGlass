//! Phase 8.2 (docs/SparkGlass_ROADMAP.md) calibration tool: "reference
//! scene + parameter sweep + side-by-side comparison + human visual
//! evaluation." This tool does the first two; a human — per the roadmap's
//! explicit instruction not to rely on one developer's memory — does the
//! last two by comparing the output against
//! `docs/references/figma-liquid-glass/` and picking a value. This tool
//! never picks a value itself.
//!
//! Renders the same 640×498 panel over `assets/image1.jpg` used by
//! `docs/references/figma-liquid-glass/liquid_glass_regular_large_640x498.png`
//! (same size, same photo) at a range of values for one parameter at a
//! time, so the candidates are directly comparable to that reference.
//! Currently sweeps the two parameters flagged unresolved in
//! `docs/references/figma-liquid-glass/README.md`: `refraction_strength`
//! and `tint_opacity`.
//!
//! Output goes to `tests/parameter_sweep/<parameter>/<value>.png`
//! (gitignored — these are exploratory, not golden references; nothing
//! here is "correct" until a human says so).
//!
//! Run with: `cargo run --example parameter_sweep`

use std::ffi::{CString, c_char, c_int, c_void};
use std::path::Path;

use glam::vec2;
use spark_glass::backend::gl::{GlGlassRenderer, upload_rgba8};
use spark_glass::glass::*;

const WIDTH: i32 = 1280;
const HEIGHT: i32 = 800;
const BACKGROUND: &[u8] = include_bytes!("../assets/image1.jpg");

// Same minimal EGL FFI as examples/visual_regression.rs — see that file for
// why (mirrors the already-proven-working sequence in c_smoke/main.c).
#[allow(non_camel_case_types)]
type EGLDisplay = *mut c_void;
#[allow(non_camel_case_types)]
type EGLConfig = *mut c_void;
#[allow(non_camel_case_types)]
type EGLSurface = *mut c_void;
#[allow(non_camel_case_types)]
type EGLContext = *mut c_void;
#[allow(non_camel_case_types)]
type EGLint = i32;
#[allow(non_camel_case_types)]
type EGLBoolean = c_int;

const EGL_DEFAULT_DISPLAY: *mut c_void = std::ptr::null_mut();
const EGL_NO_CONTEXT: EGLContext = std::ptr::null_mut();
const EGL_SURFACE_TYPE: EGLint = 0x3033;
const EGL_PBUFFER_BIT: EGLint = 0x0001;
const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
const EGL_OPENGL_ES3_BIT: EGLint = 0x0040;
const EGL_RED_SIZE: EGLint = 0x3024;
const EGL_GREEN_SIZE: EGLint = 0x3023;
const EGL_BLUE_SIZE: EGLint = 0x3022;
const EGL_ALPHA_SIZE: EGLint = 0x3021;
const EGL_NONE: EGLint = 0x3038;
const EGL_WIDTH: EGLint = 0x3057;
const EGL_HEIGHT: EGLint = 0x3056;
const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;
const EGL_OPENGL_ES_API: EGLint = 0x30A0;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetDisplay(display_id: *mut c_void) -> EGLDisplay;
    fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    fn eglChooseConfig(dpy: EGLDisplay, attrib_list: *const EGLint, configs: *mut EGLConfig, config_size: EGLint, num_config: *mut EGLint) -> EGLBoolean;
    fn eglCreatePbufferSurface(dpy: EGLDisplay, config: EGLConfig, attrib_list: *const EGLint) -> EGLSurface;
    fn eglBindAPI(api: EGLint) -> EGLBoolean;
    fn eglCreateContext(dpy: EGLDisplay, config: EGLConfig, share_context: EGLContext, attrib_list: *const EGLint) -> EGLContext;
    fn eglMakeCurrent(dpy: EGLDisplay, draw: EGLSurface, read: EGLSurface, ctx: EGLContext) -> EGLBoolean;
    fn eglGetProcAddress(procname: *const c_char) -> *const c_void;
}

unsafe fn gl_proc(name: &str) -> *const c_void {
    let cname = CString::new(name).unwrap();
    unsafe { eglGetProcAddress(cname.as_ptr()) }
}

/// The reference panel: same size as `Liquid Glass - Regular - Large` in
/// the Figma file, `preset(GlassStyle::Regular)` otherwise, with exactly
/// one field overridden per sweep step.
fn panel_with(id: u64, center: glam::Vec2, edit: impl Fn(&mut GlassMaterial, &mut GlassOptics)) -> GlassSurface {
    let (mut material, mut optics, lighting) = preset(GlassStyle::Regular, false);
    edit(&mut material, &mut optics);
    GlassSurface {
        id,
        geometry: GlassGeometry::RoundedRect { center, size: vec2(640.0, 498.0), radius: 34.0, smoothing: 0.6 },
        material,
        optics,
        lighting,
        interaction: GlassInteraction::Idle,
        style: GlassStyle::Regular,
    }
}

/// Same reference panel as `panel_with`, but with `interaction` overridden
/// instead of material/optics — Phase 9.5's glow lives on `GlassSurface`
/// directly, not inside `GlassMaterial`/`GlassOptics`, so it needs its own
/// tiny helper rather than fitting the generic `Sweep` shape below.
fn interaction_demo_panel(id: u64, center: glam::Vec2, interaction: GlassInteraction) -> GlassSurface {
    let mut surface = panel_with(id, center, |_material, _optics| {});
    surface.interaction = interaction;
    surface
}

struct Sweep {
    parameter: &'static str,
    values: &'static [f32],
    apply: fn(&mut GlassMaterial, &mut GlassOptics, f32),
}

const SWEEPS: &[Sweep] = &[
    Sweep {
        parameter: "refraction_strength",
        // 2.0 is the current working value; 70.0 (Figma's stored number)
        // would give thickness = 70*30 = 2100 — nowhere near this range,
        // deliberately not included since docs/references/figma-liquid-
        // glass/README.md already shows that's a different parameter
        // space. This brackets from the current value up to visibly
        // extreme, so a reviewer can see where it breaks down.
        values: &[1.0, 2.0, 4.0, 8.0, 16.0, 24.0],
        apply: |_material, optics, v| optics.refraction_strength = v,
    },
    Sweep {
        parameter: "tint_opacity",
        // Brackets the current Regular value (1.0), the current Control/
        // Thin value (0.15), and Figma's stored "Opacity: 25" (as 0.25) —
        // see the README's note that this one's mapping is also unproven.
        values: &[0.15, 0.25, 0.4, 0.6, 0.8, 1.0],
        apply: |material, _optics, v| material.tint_opacity = v,
    },
    Sweep {
        parameter: "clear_dimming",
        // Phase 8.8 (docs/SparkGlass_ROADMAP.md): 0 is every shipped
        // preset's default (verified as a byte-identical no-op by
        // scripts/visual_regression.sh). This brackets from off to a
        // clearly-too-strong value, so a reviewer can see where legibility
        // protection turns into just darkening the material.
        values: &[0.0, 0.2, 0.4, 0.6, 0.8, 1.0],
        apply: |material, _optics, v| material.clear_dimming = v,
    },
    Sweep {
        parameter: "adaptive_response",
        // Phase 8.5 (docs/SparkGlass_ROADMAP.md): 0 is every shipped
        // preset's default. Rendered over assets/image1.jpg (a busy photo
        // backdrop), so the rim/edge boost this drives should be visible by
        // the high end of the range.
        values: &[0.0, 0.25, 0.5, 0.75, 1.0],
        apply: |material, _optics, v| material.adaptive_response = v,
    },
];

fn main() {
    unsafe {
        let display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
        assert!(!display.is_null(), "eglGetDisplay failed");
        let mut major = 0;
        let mut minor = 0;
        assert_ne!(eglInitialize(display, &mut major, &mut minor), 0, "eglInitialize failed");

        let config_attribs = [
            EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
            EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
            EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
            EGL_NONE,
        ];
        let mut config: EGLConfig = std::ptr::null_mut();
        let mut num_configs: EGLint = 0;
        assert_ne!(eglChooseConfig(display, config_attribs.as_ptr(), &mut config, 1, &mut num_configs), 0);
        assert!(num_configs >= 1, "no EGL configs available");

        let pbuffer_attribs = [EGL_WIDTH, WIDTH, EGL_HEIGHT, HEIGHT, EGL_NONE];
        let surface = eglCreatePbufferSurface(display, config, pbuffer_attribs.as_ptr());
        assert!(!surface.is_null(), "eglCreatePbufferSurface failed");

        assert_ne!(eglBindAPI(EGL_OPENGL_ES_API), 0);
        let context_attribs = [EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE];
        let context = eglCreateContext(display, config, EGL_NO_CONTEXT, context_attribs.as_ptr());
        assert!(!context.is_null(), "eglCreateContext failed");
        assert_ne!(eglMakeCurrent(display, surface, surface, context), 0, "eglMakeCurrent failed");

        let gl = glow::Context::from_loader_function(|name| gl_proc(name));
        let mut renderer = GlGlassRenderer::new(&gl, WIDTH as f32, HEIGHT as f32);

        let image = image::load_from_memory(BACKGROUND).expect("failed to decode background").to_rgba8();
        let image = image::imageops::flip_vertical(&image);
        let background_size = vec2(image.width() as f32, image.height() as f32);
        let background = upload_rgba8(&gl, image.width() as i32, image.height() as i32, image.as_raw());

        let center = vec2(WIDTH as f32 * 0.5, HEIGHT as f32 * 0.5);

        for sweep in SWEEPS {
            let out_dir = Path::new("tests/parameter_sweep").join(sweep.parameter);
            std::fs::create_dir_all(&out_dir).expect("failed to create sweep output dir");

            for &value in sweep.values {
                renderer.draw_backdrop(&gl, background, background_size);
                let surface = panel_with(1, center, |material, optics| (sweep.apply)(material, optics, value));
                let scene = GlassScene::new(vec![surface]);
                renderer.render(&gl, &scene);
                renderer.present(&gl, WIDTH, HEIGHT);

                let path = out_dir.join(format!("{value}.png"));
                capture_png(&gl, WIDTH, HEIGHT, &path);
                println!("{} = {value} -> {}", sweep.parameter, path.display());
            }
        }

        // Phase 9.5 (docs/SparkGlass_ROADMAP.md): not a continuous sweep
        // (interaction is 5 discrete states, not a float range), so this
        // renders each state directly instead of going through `SWEEPS`.
        {
            let out_dir = Path::new("tests/parameter_sweep/interaction");
            std::fs::create_dir_all(out_dir).expect("failed to create sweep output dir");

            for state in [
                GlassInteraction::Idle,
                GlassInteraction::Hovered,
                GlassInteraction::Selected,
                GlassInteraction::Pressed,
                GlassInteraction::Dragged,
            ] {
                renderer.draw_backdrop(&gl, background, background_size);
                let surface = interaction_demo_panel(1, center, state);
                let scene = GlassScene::new(vec![surface]);
                renderer.render(&gl, &scene);
                renderer.present(&gl, WIDTH, HEIGHT);

                let path = out_dir.join(format!("{state:?}.png"));
                capture_png(&gl, WIDTH, HEIGHT, &path);
                println!("interaction = {state:?} -> {}", path.display());
            }
        }

        println!(
            "\nDone. Compare against docs/references/figma-liquid-glass/ (especially \
             liquid_glass_regular_large_640x498.png and goosic_mockup_composited_dark_bar.jpg) \
             and pick a value by eye — this tool only generates candidates, it doesn't choose one."
        );
    }
}

/// Debug/test-tooling readback only (architecture doc §43) — this whole
/// binary exists for that purpose, never used on the normal render path.
unsafe fn capture_png(gl: &glow::Context, width: i32, height: i32, path: &Path) {
    use glow::HasContext;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    unsafe {
        gl.read_pixels(0, 0, width, height, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut pixels));
    }
    let stride = (width * 4) as usize;
    let mut flipped = vec![0u8; pixels.len()];
    for row in 0..height as usize {
        let src = &pixels[row * stride..row * stride + stride];
        let dst_row = height as usize - 1 - row;
        flipped[dst_row * stride..dst_row * stride + stride].copy_from_slice(src);
    }
    image::save_buffer(path, &flipped, width as u32, height as u32, image::ColorType::Rgba8)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}
