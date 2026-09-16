//! Phase 4 (docs/SparkGlass_ROADMAP.md) visual regression capture tool.
//!
//! Renders a fixed set of named scenes headlessly (no window — a pbuffer
//! EGL context, the same pattern already proven in `c_smoke/main.c`) and
//! writes each to `tests/visual_regression/candidates/<name>.png`.
//! `scripts/visual_regression.sh` then diffs those against
//! `tests/visual_regression/golden/<name>.png` and reports pass/fail.
//!
//! This tool exists to catch *unintended* changes to the renderer's output
//! (e.g. a future Phase 8.3+ SDF change that accidentally shifts edge
//! behavior) — it says nothing about whether the current output is
//! correct against real Liquid Glass. That's a separate, human-evaluated
//! question (see docs/references/figma-liquid-glass/README.md and the
//! roadmap's Phase 8.2 calibration method).
//!
//! Run with: `cargo run --example visual_regression`

use std::ffi::{CString, c_char, c_int, c_void};
use std::path::Path;

use glam::vec2;
use spark_glass::backend::gl::{GlGlassRenderer, upload_rgba8};
use spark_glass::glass::*;

const WIDTH: i32 = 1280;
const HEIGHT: i32 = 800;

const BACKGROUNDS: [&[u8]; 4] = [
    include_bytes!("../assets/image1.jpg"),
    include_bytes!("../assets/image2.jpg"),
    include_bytes!("../assets/image3.jpg"),
    include_bytes!("../assets/image4.jpg"),
];

// Minimal EGL FFI — mirrors c_smoke/main.c's headless pbuffer setup exactly,
// so this reuses an already-verified-working sequence instead of a new,
// unverified one.
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

struct Scene {
    name: &'static str,
    background: usize,
    surfaces: Vec<GlassSurface>,
}

fn clear_profile(style: GlassStyle, dark: bool) -> (GlassMaterial, GlassOptics, GlassLighting) {
    preset(style, dark)
}

fn panel(id: u64, center: glam::Vec2, dark: bool) -> GlassSurface {
    let (material, optics, lighting) = clear_profile(GlassStyle::Regular, dark);
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

fn pill(id: u64, center: glam::Vec2) -> GlassSurface {
    let (material, optics, lighting) = clear_profile(GlassStyle::Control, false);
    GlassSurface {
        id,
        geometry: GlassGeometry::RoundedRect { center, size: vec2(380.0, 88.0), radius: 44.0, smoothing: 0.0 },
        material,
        optics,
        lighting,
        interaction: GlassInteraction::Idle,
        style: GlassStyle::Control,
    }
}

fn scenes() -> Vec<Scene> {
    let (w, h) = (WIDTH as f32, HEIGHT as f32);
    vec![
        Scene {
            name: "panel_only_bg1",
            background: 0,
            surfaces: vec![panel(1, vec2(w * 0.5, h * 0.5), false)],
        },
        Scene {
            name: "pill_only_bg1",
            background: 0,
            surfaces: vec![pill(1, vec2(w * 0.5, h * 0.5))],
        },
        Scene {
            name: "panel_and_pill_bg1",
            background: 0,
            surfaces: vec![panel(1, vec2(w * 0.5, h * 0.4), false), pill(2, vec2(w * 0.5, h * 0.86))],
        },
        Scene {
            name: "panel_and_pill_bg2",
            background: 1,
            surfaces: vec![panel(1, vec2(w * 0.5, h * 0.4), false), pill(2, vec2(w * 0.5, h * 0.86))],
        },
        Scene {
            name: "panel_and_pill_bg3",
            background: 2,
            surfaces: vec![panel(1, vec2(w * 0.5, h * 0.4), false), pill(2, vec2(w * 0.5, h * 0.86))],
        },
        Scene {
            name: "panel_and_pill_bg4",
            background: 3,
            surfaces: vec![panel(1, vec2(w * 0.5, h * 0.4), false), pill(2, vec2(w * 0.5, h * 0.86))],
        },
        Scene {
            name: "dark_tint_panel_bg1",
            background: 0,
            surfaces: vec![panel(1, vec2(w * 0.5, h * 0.5), true)],
        },
        Scene {
            name: "overlapping_panels_bg2",
            background: 1,
            // Deliberately overlapping — exercises the glass-on-glass stack
            // response in glass.frag (the surface below shows through the
            // one above, per stack_response in layer_glass()).
            surfaces: vec![panel(1, vec2(w * 0.42, h * 0.5), false), panel(2, vec2(w * 0.58, h * 0.5), false)],
        },
    ]
}

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
        assert_ne!(
            eglChooseConfig(display, config_attribs.as_ptr(), &mut config, 1, &mut num_configs),
            0
        );
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

        let mut background_textures = Vec::new();
        for bytes in BACKGROUNDS {
            let image = image::load_from_memory(bytes).expect("failed to decode background").to_rgba8();
            let image = image::imageops::flip_vertical(&image);
            let size = vec2(image.width() as f32, image.height() as f32);
            let texture = upload_rgba8(&gl, image.width() as i32, image.height() as i32, image.as_raw());
            background_textures.push((texture, size));
        }

        let out_dir = Path::new("tests/visual_regression/candidates");
        std::fs::create_dir_all(out_dir).expect("failed to create candidates dir");

        let all_scenes = scenes();
        let scene_count = all_scenes.len();
        for scene_def in all_scenes {
            let (texture, size) = background_textures[scene_def.background];
            renderer.draw_backdrop(&gl, texture, size);

            let mut scene = GlassScene::new(scene_def.surfaces);
            scene.quality = GlassQuality::High;
            renderer.render(&gl, &scene);
            renderer.present(&gl, WIDTH, HEIGHT);

            let path = out_dir.join(format!("{}.png", scene_def.name));
            capture_png(&gl, WIDTH, HEIGHT, &path);
            println!("captured {}", scene_def.name);
        }

        println!("Done — {scene_count} scenes written to {}", out_dir.display());
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
