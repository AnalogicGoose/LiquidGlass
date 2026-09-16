//! Phase 2 exit-condition proof: drives SparkGlass purely through the
//! `extern "C"` functions in `src/ffi/`, the same way a C/C++ host would —
//! no `GlGlassRenderer`, no `GlassScene`, just the opaque `SparkGlassContext`
//! pointer and POD structs. If this compiles and renders correctly, the ABI
//! boundary genuinely carries no Rust-specific requirement.
//!
//! Windowing/context setup is intentionally identical to `examples/sandbox.rs`
//! — only the render calls differ. Run with: `cargo run --example ffi_smoke`

use std::ffi::CString;
use std::num::NonZeroU32;
use std::os::raw::{c_char, c_void};
use std::sync::OnceLock;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{Display, GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use spark_glass::backend::gl::upload_rgba8;
use spark_glass::ffi::{
    SGFrame, SGGlassElement, SGQuality, SGTextureHandle, SparkGlassContext, sg_create, sg_destroy, sg_import_gl_texture, sg_present,
    sg_render_frame, sg_resize, sg_set_backdrop,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const BACKGROUND: &[u8] = include_bytes!("../assets/image1.jpg");

/// `extern "C" fn` pointers cannot capture state, so the GL display used to
/// resolve proc addresses is stashed here — exactly the kind of thing a C
/// host would do with a global/thread-local instead of a closure.
static GL_DISPLAY: OnceLock<Display> = OnceLock::new();

unsafe extern "C" fn resolve_gl_proc(name: *const c_char) -> *const c_void {
    let display = GL_DISPLAY.get().expect("GL display not initialized before sg_create");
    unsafe { display.get_proc_address(std::ffi::CStr::from_ptr(name)) as *const c_void }
}

struct AppState {
    window: Window,
    gl_surface: Surface<WindowSurface>,
    gl_context: PossiblyCurrentContext,
    ctx: *mut SparkGlassContext,
    background: SGTextureHandle,
    frame: u64,
}

impl Drop for AppState {
    fn drop(&mut self) {
        unsafe { sg_destroy(self.ctx) };
    }
}

#[derive(Default)]
struct App {
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = WindowAttributes::default()
            .with_title("SparkGlass — FFI smoke test (extern \"C\" boundary)")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));

        let template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));
        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|best, config| if config.num_samples() > best.num_samples() { config } else { best })
                    .expect("no GL configs available")
            })
            .expect("failed to build window/GL config");
        let window = window.expect("failed to create window");

        let raw_window_handle = window.window_handle().ok().map(|h| h.as_raw());
        let gl_display = gl_config.display();
        GL_DISPLAY.set(gl_display.clone()).ok();

        let gles_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))))
            .build(raw_window_handle);
        let fallback_attrs = ContextAttributesBuilder::new().build(raw_window_handle);
        let not_current_context = unsafe {
            gl_display
                .create_context(&gl_config, &gles_attrs)
                .or_else(|_| gl_display.create_context(&gl_config, &fallback_attrs))
                .expect("failed to create GL context")
        };

        let surface_attrs = window.build_surface_attributes(Default::default()).expect("failed to build surface attributes");
        let gl_surface = unsafe { gl_display.create_window_surface(&gl_config, &surface_attrs).expect("failed to create window surface") };
        let gl_context = not_current_context.make_current(&gl_surface).expect("failed to make GL context current");
        let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));

        // A local `glow::Context` purely to upload the demo background asset
        // — a real host would use whatever it already uses for its own GPU
        // resources. SparkGlass's own `glow::Context` lives behind `ctx` and
        // is never touched directly here.
        let asset_gl = unsafe { glow::Context::from_loader_function(|name| resolve_gl_proc(CString::new(name).unwrap().as_ptr())) };
        let image = image::load_from_memory(BACKGROUND).expect("failed to decode background").to_rgba8();
        let image = image::imageops::flip_vertical(&image);
        let background = unsafe { upload_rgba8(&asset_gl, image.width() as i32, image.height() as i32, image.as_raw()) };

        let size = window.inner_size();
        let ctx = unsafe { sg_create(resolve_gl_proc, size.width as f32, size.height as f32) };
        assert!(!ctx.is_null(), "sg_create failed");
        let background_handle = unsafe { sg_import_gl_texture(ctx, background.0.get(), image.width() as f32, image.height() as f32) };
        assert_ne!(background_handle, 0, "sg_import_gl_texture failed");
        let backdrop_result = unsafe { sg_set_backdrop(ctx, background_handle) };
        assert_eq!(backdrop_result as i32, 0, "sg_set_backdrop failed");

        self.state = Some(AppState {
            window,
            gl_surface,
            gl_context,
            ctx,
            background: background_handle,
            frame: 0,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state.gl_surface.resize(&state.gl_context, NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap());
                unsafe { sg_resize(state.ctx, size.width as f32, size.height as f32) };
                unsafe { sg_set_backdrop(state.ctx, state.background) };
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.frame += 1;
                let size = state.window.inner_size();
                let (w, h) = (size.width as f32, size.height as f32);

                let elements = [
                    SGGlassElement {
                        id: 1,
                        center_x: w * 0.5,
                        center_y: h * 0.4,
                        size_x: 640.0,
                        size_y: 498.0,
                        radius: 34.0,
                        smoothing: 0.6,
                        refraction_strength: 2.0,
                        depth: 30.0,
                        dispersion: 0.2,
                        frost_radius: 6.0,
                        light_intensity: 0.25,
                        light_angle_degrees: 0.0,
                        light_splay: 0.2,
                        tint_opacity: 0.15,
                        dark_tint: 0,
                        shadow_strength: 1.0,
                    },
                    SGGlassElement {
                        id: 2,
                        center_x: w * 0.5,
                        center_y: h * 0.86,
                        size_x: 380.0,
                        size_y: 88.0,
                        radius: 44.0,
                        smoothing: 0.0,
                        refraction_strength: 2.0,
                        depth: 30.0,
                        dispersion: 0.2,
                        frost_radius: 6.0,
                        light_intensity: 0.25,
                        light_angle_degrees: 0.0,
                        light_splay: 0.2,
                        tint_opacity: 0.15,
                        dark_tint: 0,
                        shadow_strength: 1.0,
                    },
                ];

                let frame = SGFrame {
                    struct_size: std::mem::size_of::<SGFrame>(),
                    width: w,
                    height: h,
                    quality: SGQuality::High,
                    reduced_transparency: 0,
                    reduced_motion: 0,
                    elements: elements.as_ptr(),
                    element_count: elements.len(),
                };

                let render_result = unsafe { sg_render_frame(state.ctx, &frame) };
                assert_eq!(render_result as i32, 0, "sg_render_frame failed");
                let present_result = unsafe { sg_present(state.ctx, size.width as i32, size.height as i32) };
                assert_eq!(present_result as i32, 0, "sg_present failed");

                if let Ok(path) = std::env::var("SPARK_GLASS_SANDBOX_CAPTURE")
                    && state.frame == 5
                {
                    capture_png(size.width, size.height, &path);
                    event_loop.exit();
                    return;
                }

                state.gl_surface.swap_buffers(&state.gl_context).expect("failed to swap buffers");
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn capture_png(width: u32, height: u32, path: &str) {
    use glow::HasContext;
    let gl = unsafe { glow::Context::from_loader_function(|name| resolve_gl_proc(CString::new(name).unwrap().as_ptr())) };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    unsafe {
        gl.read_pixels(0, 0, width as i32, height as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut pixels));
    }
    let stride = (width * 4) as usize;
    let mut flipped = vec![0u8; pixels.len()];
    for row in 0..height as usize {
        let src = &pixels[row * stride..row * stride + stride];
        let dst_row = height as usize - 1 - row;
        flipped[dst_row * stride..dst_row * stride + stride].copy_from_slice(src);
    }
    image::save_buffer(path, &flipped, width, height, image::ColorType::Rgba8).expect("failed to write capture PNG");
}

fn main() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("event loop error");
}
