//! Empirical answer to the architecture doc's OPEN "GL State Ownership"
//! question (§37): does GTK's own widget compositing (GSK) survive our GL
//! calls leaving VAO/program/blend/texture-unit state dirty, with no
//! save/restore on our side? `examples/gtk_glarea.rs` only ever has the
//! `GLArea` on screen, so it can't answer that. This variant puts real
//! native GTK widgets — a `Button`, a `Switch`, a `Label` — in a
//! `gtk::Overlay` on top of the same `GLArea`, so their rendering runs
//! immediately after ours on every frame.
//!
//! Per the doc's Rule 3 ("if OPEN, don't silently pick one — run the
//! experiment"): this is that experiment, not a claim that the answer is
//! settled. Run it and look — if the button/switch/label render crisply
//! with no artifacts, that is evidence a minimal/no state contract may be
//! enough on GTK; if they glitch, that's evidence the other way.
//!
//! Run with: `cargo run --example gtk_glarea_overlay`

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Button, GLArea, Label, Overlay, Switch};
use spark_glass::backend::gl::{GlGlassRenderer, upload_rgba8};
use spark_glass::glass::*;

const BACKGROUND: &[u8] = include_bytes!("../assets/image1.jpg");
const APP_ID: &str = "org.analogicgoose.sparkglass.GtkGlAreaOverlaySandbox";

struct GlState {
    gl: glow::Context,
    renderer: GlGlassRenderer,
    background: glow::NativeTexture,
    background_size: glam::Vec2,
}

struct GlLoader {
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

    GlassScene::new(vec![GlassSurface {
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
    }])
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
            let loader = GlLoader::load();
            let gl = unsafe { glow::Context::from_loader_function(|name| loader.get(name)) };
            let renderer = GlGlassRenderer::new(&gl, width, height);
            let image = image::load_from_memory(BACKGROUND).expect("failed to decode background").to_rgba8();
            let image = image::imageops::flip_vertical(&image);
            let background_size = glam::vec2(image.width() as f32, image.height() as f32);
            let background = unsafe { upload_rgba8(&gl, image.width() as i32, image.height() as i32, image.as_raw()) };
            GlState { gl, renderer, background, background_size }
        });

        state.renderer.resize_if_needed(&state.gl, width, height);
        state.renderer.draw_backdrop(&state.gl, state.background, state.background_size);
        let scene = build_scene(width, height);
        state.renderer.render(&state.gl, &scene);
        state.renderer.present(&state.gl, width as i32, height as i32);

        gtk4::glib::Propagation::Stop
    });

    let redraw_area = gl_area.clone();
    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        redraw_area.queue_render();
        gtk4::glib::ControlFlow::Continue
    });

    // Real native GTK widgets, laid out by GTK itself (not by us), composited
    // by GSK on top of the GLArea's rendered texture every frame. Nothing
    // about their rendering goes through SparkGlass or our GL calls.
    let button = Button::with_label("Native GTK Button");
    button.set_halign(gtk4::Align::Start);
    button.set_valign(gtk4::Align::Start);
    button.set_margin_start(24);
    button.set_margin_top(24);

    let switch = Switch::new();
    switch.set_active(true);
    switch.set_halign(gtk4::Align::End);
    switch.set_valign(gtk4::Align::Start);
    switch.set_margin_end(24);
    switch.set_margin_top(24);

    let label = Label::new(Some("Native GTK Label composited over the glow renderer"));
    label.set_halign(gtk4::Align::Center);
    label.set_valign(gtk4::Align::End);
    label.set_margin_bottom(24);
    label.add_css_class("title-2");

    let overlay = Overlay::new();
    overlay.set_child(Some(&gl_area));
    overlay.add_overlay(&button);
    overlay.add_overlay(&switch);
    overlay.add_overlay(&label);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("SparkGlass — GTK4 native widgets over GLArea (GL state experiment)")
        .default_width(1280)
        .default_height(800)
        .child(&overlay)
        .build();
    window.present();
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run_with_args::<&str>(&[]);
}
