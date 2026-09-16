//! Phase 4 windowless-core proof: the same SparkGlass material rendered
//! through `winit` + `glutin` + `glow`, with no Macroquad anywhere in this
//! binary. This is the development sandbox described in
//! `docs/SparkGlass_MASTER_ARCHITECTURE.md` (§45) — it owns the window and
//! event loop, which the production core never will.
//!
//! Run with: `cargo run --example sandbox`

use std::ffi::CString;
use std::num::NonZeroU32;

use glam::{Vec2, vec2};
use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use spark_glass::backend::gl::{GlGlassRenderer, upload_rgba8};
use spark_glass::glass::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const BACKGROUND: &[u8] = include_bytes!("../assets/image1.jpg");

struct AppState {
    window: Window,
    gl_surface: Surface<WindowSurface>,
    gl_context: PossiblyCurrentContext,
    gl: glow::Context,
    renderer: GlGlassRenderer,
    background: glow::NativeTexture,
    background_size: Vec2,
    scene: GlassScene,
    frame: u64,
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
            .with_title("SparkGlass — windowless-core sandbox (glow backend)")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));

        let template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));
        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|best, config| {
                        if config.num_samples() > best.num_samples() {
                            config
                        } else {
                            best
                        }
                    })
                    .expect("no GL configs available")
            })
            .expect("failed to build window/GL config");
        let window = window.expect("failed to create window");

        let raw_window_handle = window.window_handle().ok().map(|h| h.as_raw());
        let gl_display = gl_config.display();

        // Per the architecture's GLES-baseline direction (§8): ask for GLES
        // 3.0 first, fall back to whatever desktop GL the driver offers.
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

        let surface_attrs = window
            .build_surface_attributes(Default::default())
            .expect("failed to build surface attributes");
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &surface_attrs)
                .expect("failed to create window surface")
        };
        let gl_context = not_current_context
            .make_current(&gl_surface)
            .expect("failed to make GL context current");
        let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));

        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                let name = CString::new(name).unwrap();
                gl_display.get_proc_address(&name) as *const _
            })
        };

        let size = window.inner_size();
        let renderer = GlGlassRenderer::new(&gl, size.width as f32, size.height as f32);

        // Every other texture this renderer samples is FBO-rendered, where a
        // GL quirk means v=0 ends up at the *bottom* of what was drawn (see
        // the vertex shader's uv flip). A plain CPU upload has no such
        // rasterization step, so row 0 of the source image lands at v=0
        // directly. Flipping once here keeps this texture consistent with
        // that same convention, instead of special-casing the backdrop draw.
        let image = image::load_from_memory(BACKGROUND)
            .expect("failed to decode sandbox background")
            .to_rgba8();
        let image = image::imageops::flip_vertical(&image);
        let background_size = vec2(image.width() as f32, image.height() as f32);
        let background = unsafe {
            upload_rgba8(&gl, image.width() as i32, image.height() as i32, image.as_raw())
        };
        renderer.draw_backdrop(&gl, background, background_size);

        let (w, h) = (size.width as f32, size.height as f32);
        // Phase 8.1 (docs/SparkGlass_ROADMAP.md): each surface gets its own
        // style's preset instead of one profile stamped onto every surface
        // — a large Regular panel and a small Control pill are not the same
        // material merely scaled to different dimensions.
        let (panel_material, panel_optics, panel_lighting) = preset(GlassStyle::Regular, false);
        let (pill_material, pill_optics, pill_lighting) = preset(GlassStyle::Control, false);
        let scene = GlassScene::new(vec![
            GlassSurface {
                id: 1,
                geometry: GlassGeometry::RoundedRect {
                    center: vec2(w * 0.5, h * 0.4),
                    size: vec2(640.0, 498.0),
                    radius: 34.0,
                    smoothing: 0.6,
                },
                material: panel_material,
                optics: panel_optics,
                lighting: panel_lighting,
                interaction: GlassInteraction::Idle,
                style: GlassStyle::Regular,
            },
            GlassSurface {
                id: 2,
                geometry: GlassGeometry::RoundedRect {
                    center: vec2(w * 0.5, h * 0.86),
                    size: vec2(380.0, 88.0),
                    radius: 44.0,
                    smoothing: 0.0,
                },
                material: pill_material,
                optics: pill_optics,
                lighting: pill_lighting,
                interaction: GlassInteraction::Idle,
                style: GlassStyle::Control,
            },
        ]);

        self.state = Some(AppState {
            window,
            gl_surface,
            gl_context,
            gl,
            renderer,
            background,
            background_size,
            scene,
            frame: 0,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state
                    .gl_surface
                    .resize(&state.gl_context, NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap());
                state.renderer.resize_if_needed(&state.gl, size.width as f32, size.height as f32);
                state.renderer.draw_backdrop(&state.gl, state.background, state.background_size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.scene.frame += 1;
                state.frame += 1;
                state.renderer.render(&state.gl, &state.scene);

                let size = state.window.inner_size();
                state.renderer.present(&state.gl, size.width as i32, size.height as i32);

                // Debug/test-tooling only readback (architecture doc §43
                // explicitly allows this for screenshot capture; it never
                // runs in the normal render path).
                if let Ok(path) = std::env::var("SPARK_GLASS_SANDBOX_CAPTURE")
                    && state.frame == 5
                {
                    capture_png(&state.gl, size.width, size.height, &path);
                    event_loop.exit();
                    return;
                }

                state
                    .gl_surface
                    .swap_buffers(&state.gl_context)
                    .expect("failed to swap buffers");
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

/// Test-tooling readback only — never called from the normal render path.
fn capture_png(gl: &glow::Context, width: u32, height: u32, path: &str) {
    use glow::HasContext;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    unsafe {
        gl.read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut pixels),
        );
    }
    // glReadPixels rows start at the bottom; flip to a top-down PNG.
    let stride = (width * 4) as usize;
    let mut flipped = vec![0u8; pixels.len()];
    for row in 0..height as usize {
        let src = &pixels[row * stride..row * stride + stride];
        let dst_row = height as usize - 1 - row;
        flipped[dst_row * stride..dst_row * stride + stride].copy_from_slice(src);
    }
    image::save_buffer(path, &flipped, width, height, image::ColorType::Rgba8)
        .expect("failed to write sandbox capture PNG");
}

fn main() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("event loop error");
}
