//! Phase 5 proof: SparkGlass embedded inside a real GTK4 `GtkGLArea`,
//! exactly the integration model described in the architecture doc (§5.2,
//! §49, §50). GTK owns the widget, the `GdkGLContext`, and the render
//! callback's framebuffer; SparkGlass only ever uses the context GTK makes
//! current for us and draws into whichever framebuffer GTK already bound —
//! it never creates a window, context, or second GL surface of its own.
//!
//! Run with: `cargo run --example gtk_glarea`

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, GLArea};
use spark_glass_poc::backend::gl::{GlGlassRenderer, upload_rgba8};
use spark_glass_poc::glass::*;

const BACKGROUND: &[u8] = include_bytes!("../assets/image1.jpg");
const APP_ID: &str = "org.analogicgoose.sparkglass.GtkGlAreaSandbox";

/// Everything that depends on a current GL context, created lazily on the
/// first `render` signal — GTK does not guarantee a context is current
/// before that point.
struct GlState {
    gl: glow::Context,
    renderer: GlGlassRenderer,
    background: glow::NativeTexture,
    background_size: glam::Vec2,
    frame: u64,
}

/// Debug/test-tooling readback only (architecture doc §43) — dumps whatever
/// is in the currently bound framebuffer, which GTK already populated via
/// `present`. Never used in the normal render path.
fn capture_png(gl: &glow::Context, width: i32, height: i32, path: &str) {
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
    image::save_buffer(path, &flipped, width as u32, height as u32, image::ColorType::Rgba8).expect("failed to write capture PNG");
}

/// Resolves GL function addresses via `eglGetProcAddress` — the same
/// mechanism `glutin`'s EGL backend uses in the other examples, sourced
/// directly here because gtk4-rs's `GdkGLContext` does not expose a loader
/// of its own. GTK made its context current on this thread before invoking
/// the `render` signal, and `eglGetProcAddress` resolves against whatever
/// context is current process-wide, so this works regardless of which
/// library created that context.
struct GlLoader {
    // Kept alive for the process lifetime; `get_proc_address` borrows it.
    _egl: libloading::Library,
    get_proc_address: unsafe extern "C" fn(*const std::os::raw::c_char) -> *const std::ffi::c_void,
}

impl GlLoader {
    fn load() -> Self {
        let egl = unsafe { libloading::Library::new("libEGL.so.1") }.expect("libEGL.so.1 not found");
        let get_proc_address = *unsafe {
            egl.get::<unsafe extern "C" fn(*const std::os::raw::c_char) -> *const std::ffi::c_void>(b"eglGetProcAddress\0")
        }
        .expect("eglGetProcAddress not found in libEGL");
        Self { _egl: egl, get_proc_address }
    }

    fn get(&self, name: &str) -> *const std::ffi::c_void {
        let Ok(cname) = std::ffi::CString::new(name) else { return std::ptr::null() };
        unsafe { (self.get_proc_address)(cname.as_ptr()) }
    }
}

fn build_scene(width: f32, height: f32) -> GlassScene {
    // Matches the "Clear" profile in main.rs, for the same reason as the
    // other examples: directly comparable captures across every backend.
    let (mut material, mut optics, mut lighting) = preset(GlassStyle::Regular, false);
    material.frost_radius = 6.0;
    material.tint_opacity = 0.15;
    material.dark_tint = false;
    optics.refraction_strength = 2.0;
    optics.depth = 30.0;
    optics.dispersion = 0.2;
    lighting.intensity = 0.25;
    lighting.angle_degrees = 0.0;
    lighting.splay = 0.2;
    lighting.shadow_strength = 1.0;

    GlassScene::new(vec![
        GlassSurface {
            id: 1,
            geometry: GlassGeometry::RoundedRect {
                center: glam::vec2(width * 0.5, height * 0.4),
                size: glam::vec2(640.0, 498.0),
                radius: 34.0,
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
                center: glam::vec2(width * 0.5, height * 0.86),
                size: glam::vec2(380.0, 88.0),
                radius: 44.0,
                smoothing: 0.0,
            },
            material,
            optics,
            lighting,
            interaction: GlassInteraction::Idle,
            style: GlassStyle::Control,
        },
    ])
}

fn build_ui(app: &Application) {
    let gl_area = GLArea::new();
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);

    let state: Rc<RefCell<Option<GlState>>> = Rc::new(RefCell::new(None));

    gl_area.connect_render(move |area, _gdk_context| {
        let scale = area.scale_factor().max(1) as f32;
        let width = area.width() as f32 * scale;
        let height = area.height() as f32 * scale;
        if width <= 0.0 || height <= 0.0 {
            return gtk4::glib::Propagation::Stop;
        }

        let mut state = state.borrow_mut();
        let state = state.get_or_insert_with(|| {
            // GTK made its GdkGLContext current before invoking this signal
            // (that is the whole point of GtkGLArea) — the loader resolves
            // against whatever context is current on this thread right now.
            let loader = GlLoader::load();
            let gl = unsafe { glow::Context::from_loader_function(|name| loader.get(name)) };
            let renderer = GlGlassRenderer::new(&gl, width, height);
            let image = image::load_from_memory(BACKGROUND).expect("failed to decode background").to_rgba8();
            let image = image::imageops::flip_vertical(&image);
            let background_size = glam::vec2(image.width() as f32, image.height() as f32);
            let background = unsafe { upload_rgba8(&gl, image.width() as i32, image.height() as i32, image.as_raw()) };
            GlState {
                gl,
                renderer,
                background,
                background_size,
                frame: 0,
            }
        });

        state.frame += 1;
        state.renderer.resize_if_needed(&state.gl, width, height);
        state.renderer.draw_backdrop(&state.gl, state.background, state.background_size);
        let scene = build_scene(width, height);
        state.renderer.render(&state.gl, &scene);
        // GtkGLArea already bound its own framebuffer for this callback —
        // `present` never rebinds, it only draws into whatever is current,
        // exactly the "operate inside the host's render callback" rule.
        state.renderer.present(&state.gl, width as i32, height as i32);

        if let Ok(path) = std::env::var("SPARK_GLASS_SANDBOX_CAPTURE")
            && state.frame == 5
        {
            capture_png(&state.gl, width as i32, height as i32, &path);
            std::process::exit(0);
        }

        gtk4::glib::Propagation::Stop
    });

    // GtkGLArea only redraws on demand; keep it ticking so a capture request
    // (or just watching the window) actually gets frames.
    let redraw_area = gl_area.clone();
    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        redraw_area.queue_render();
        gtk4::glib::ControlFlow::Continue
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .title("SparkGlass — GTK4 GtkGLArea integration")
        .default_width(1280)
        .default_height(800)
        .child(&gl_area)
        .build();
    window.present();
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    // Run under GTK's own arg parsing with no argv, so `cargo run --example
    // gtk_glarea -- --foo` doesn't confuse GApplication's option parser.
    app.run_with_args::<&str>(&[]);
}
