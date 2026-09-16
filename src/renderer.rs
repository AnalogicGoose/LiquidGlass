//! Renderer boundary. `MacroquadGlassRenderer` is the current reference
//! implementation; WinUI and GTK adapters can consume the exact same types in
//! `glass` without leaking platform effects into application code.
#![allow(dead_code)] // Native adapter contracts are intentionally not linked by this PoC.

use crate::glass::{GlassQuality, GlassScene, GlassSurface, interaction_energy};
use macroquad::prelude::*;

pub const SHADOW_MARGIN: f32 = 96.0;

pub const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying vec2 v_px;
varying vec2 v_uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() { v_px = position.xy; v_uv = texcoord; gl_Position = Projection * Model * vec4(position, 1.0); }
"#;

pub struct SceneTargets {
    pub sharp: RenderTarget,
    pub blur: RenderTarget,
    pub blur_tmp: RenderTarget,
    /// Full-resolution ping-pong targets containing the ordered glass stack.
    /// A surface reads one while writing the next, so overlapping surfaces see
    /// the glass already rendered behind them without a GPU readback.
    pub stack_a: RenderTarget,
    pub stack_b: RenderTarget,
    /// Union coverage of surfaces already present in the ordered stack. The
    /// next surface samples it to select its glass-on-glass optical response.
    pub mask_a: RenderTarget,
    pub mask_b: RenderTarget,
    /// Frosted accumulated coverage. This softens the lower surface's inner
    /// edge response together with its colour at high frost values.
    pub mask_blur: RenderTarget,
    pub mask_blur_tmp: RenderTarget,
}

impl SceneTargets {
    pub fn new(width: f32, height: f32) -> Self {
        let target = |w: u32, h: u32| {
            let t = render_target(w.max(1), h.max(1));
            t.texture.set_filter(FilterMode::Linear);
            t
        };
        let (w, h) = (width as u32, height as u32);
        Self {
            sharp: target(w, h),
            blur: target(w / 2, h / 2),
            blur_tmp: target(w / 2, h / 2),
            stack_a: target(w, h),
            stack_b: target(w, h),
            mask_a: target(w, h),
            mask_b: target(w, h),
            mask_blur: target(w / 2, h / 2),
            mask_blur_tmp: target(w / 2, h / 2),
        }
    }
}

pub struct MacroquadGlassRenderer {
    glass: Material,
    blur: Material,
    mask: Material,
    pub targets: SceneTargets,
}

impl MacroquadGlassRenderer {
    pub fn new(width: f32, height: f32) -> Self {
        let glass = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX_SHADER,
                fragment: include_str!("glass.frag"),
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(miniquad::BlendState::new(
                        miniquad::Equation::Add,
                        miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                        miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha),
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
                    UniformDesc::new("u_saturation", UniformType::Float1),
                    UniformDesc::new("u_brightness", UniformType::Float1),
                    UniformDesc::new("u_contrast", UniformType::Float1),
                    UniformDesc::new("u_clear_dimming", UniformType::Float1),
                    UniformDesc::new("u_adaptive_response", UniformType::Float1),
                    UniformDesc::new("u_ambient_reflection", UniformType::Float1),
                    UniformDesc::new("u_interaction", UniformType::Float1),
                ],
                textures: vec![
                    "u_scene".to_owned(),
                    "u_scene_blur".to_owned(),
                    "u_stack_mask".to_owned(),
                ],
            },
        )
        .expect("SparkGlass shader must compile");
        let blur = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX_SHADER,
                fragment: include_str!("blur.frag"),
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
        .expect("Blur shader must compile");
        let mask = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX_SHADER,
                fragment: include_str!("glass_mask.frag"),
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("u_center", UniformType::Float2),
                    UniformDesc::new("u_size", UniformType::Float2),
                    UniformDesc::new("u_radius", UniformType::Float1),
                    UniformDesc::new("u_smoothing", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .expect("Glass coverage shader must compile");
        Self {
            glass,
            blur,
            mask,
            targets: SceneTargets::new(width, height),
        }
    }

    pub fn resize_if_needed(&mut self, width: f32, height: f32) {
        if self.targets.sharp.texture.size() != vec2(width, height) {
            self.targets = SceneTargets::new(width, height);
        }
    }

    pub fn begin_backdrop(&self, draw: impl FnOnce()) {
        set_target_camera(&self.targets.sharp);
        clear_background(BLACK);
        draw();
    }

    pub fn render(&mut self, scene: &GlassScene, width: f32, height: f32) {
        let frame_size = vec2(width, height);
        set_target_camera(&self.targets.stack_a);
        clear_background(BLACK);
        blit(&self.targets.sharp.texture, frame_size);
        set_target_camera(&self.targets.mask_a);
        clear_background(BLACK);

        let mut current = self.targets.stack_a.clone();
        let mut next = self.targets.stack_b.clone();
        let mut current_mask = self.targets.mask_a.clone();
        let mut next_mask = self.targets.mask_b.clone();
        self.glass.set_uniform("u_resolution", vec2(width, height));

        for surface in &scene.surfaces {
            // Blur the accumulated scene, not only the original backdrop. This
            // is what lets a top glass surface diffuse a lower glass surface.
            let frost = surface_frost(surface, scene.quality, scene.reduced_transparency);
            blur_source(
                &self.blur,
                &current.texture,
                &self.targets.blur,
                &self.targets.blur_tmp,
                frost * 0.5,
            );
            blur_source(
                &self.blur,
                &current_mask.texture,
                &self.targets.mask_blur,
                &self.targets.mask_blur_tmp,
                frost * 0.5,
            );
            self.glass
                .set_texture("u_scene_blur", self.targets.blur.texture.clone());

            set_target_camera(&next);
            clear_background(BLACK);
            blit(&current.texture, frame_size);
            self.glass.set_texture("u_scene", current.texture.clone());
            self.glass
                .set_texture("u_stack_mask", self.targets.mask_blur.texture.clone());
            self.draw_surface(surface, scene.quality, scene.reduced_transparency);

            set_target_camera(&next_mask);
            clear_background(BLACK);
            blit(&current_mask.texture, frame_size);
            self.draw_mask(surface);

            std::mem::swap(&mut current, &mut next);
            std::mem::swap(&mut current_mask, &mut next_mask);
        }

        set_default_camera();
        clear_background(BLACK);
        blit(&current.texture, frame_size);
    }

    fn draw_surface(
        &mut self,
        surface: &GlassSurface,
        quality: GlassQuality,
        reduced_transparency: bool,
    ) {
        let geometry = surface.geometry;
        let frost = surface_frost(surface, quality, reduced_transparency);
        self.glass.set_uniform("u_center", geometry.center());
        self.glass.set_uniform("u_size", geometry.size());
        self.glass.set_uniform("u_radius", geometry.radius());
        self.glass.set_uniform("u_smoothing", geometry.smoothing());
        self.glass.set_uniform(
            "u_refraction",
            if matches!(quality, GlassQuality::Low | GlassQuality::Fallback) {
                0.0
            } else {
                surface.optics.refraction_strength
            },
        );
        self.glass.set_uniform("u_depth", surface.optics.depth);
        self.glass
            .set_uniform("u_dispersion", surface.optics.dispersion);
        self.glass.set_uniform("u_frost", frost);
        self.glass
            .set_uniform("u_light_intensity", surface.lighting.intensity);
        self.glass
            .set_uniform("u_light_angle", surface.lighting.angle_degrees);
        self.glass.set_uniform("u_splay", surface.lighting.splay);
        self.glass.set_uniform(
            "u_tint_mode",
            if surface.material.dark_tint {
                1.0f32
            } else {
                0.0f32
            },
        );
        self.glass
            .set_uniform("u_tint", surface.material.tint_opacity);
        self.glass
            .set_uniform("u_shadow", surface.lighting.shadow_strength);
        self.glass
            .set_uniform("u_saturation", surface.material.saturation);
        self.glass
            .set_uniform("u_brightness", surface.material.brightness);
        self.glass
            .set_uniform("u_contrast", surface.material.contrast);
        self.glass
            .set_uniform("u_clear_dimming", surface.material.clear_dimming);
        self.glass
            .set_uniform("u_adaptive_response", surface.material.adaptive_response);
        self.glass
            .set_uniform("u_ambient_reflection", surface.material.ambient_reflection);
        self.glass
            .set_uniform("u_interaction", interaction_energy(surface.interaction));
        gl_use_material(&self.glass);
        let top_left = geometry.center() - geometry.size() * 0.5 - SHADOW_MARGIN;
        let quad = geometry.size() + SHADOW_MARGIN * 2.0;
        draw_rectangle(top_left.x, top_left.y, quad.x, quad.y, WHITE);
        gl_use_default_material();
    }

    fn draw_mask(&mut self, surface: &GlassSurface) {
        let geometry = surface.geometry;
        self.mask.set_uniform("u_center", geometry.center());
        self.mask.set_uniform("u_size", geometry.size());
        self.mask.set_uniform("u_radius", geometry.radius());
        self.mask.set_uniform("u_smoothing", geometry.smoothing());
        gl_use_material(&self.mask);
        let top_left = geometry.center() - geometry.size() * 0.5 - 1.0;
        let quad = geometry.size() + 2.0;
        draw_rectangle(top_left.x, top_left.y, quad.x, quad.y, WHITE);
        gl_use_default_material();
    }
}

fn surface_frost(surface: &GlassSurface, quality: GlassQuality, reduced_transparency: bool) -> f32 {
    if reduced_transparency || matches!(quality, GlassQuality::Fallback) {
        0.0
    } else {
        surface.material.frost_radius
    }
}

fn blur_source(
    material: &Material,
    source: &Texture2D,
    output: &RenderTarget,
    scratch: &RenderTarget,
    sigma: f32,
) {
    let half = output.texture.size();
    set_target_camera(output);
    blit(source, half);
    material.set_uniform("u_texel", 1.0 / half);
    material.set_uniform("u_sigma", sigma * 0.5);
    for (source, dest, direction) in [
        (output, scratch, vec2(1.0, 0.0)),
        (scratch, output, vec2(0.0, 1.0)),
    ] {
        set_target_camera(dest);
        material.set_uniform("u_direction", direction);
        gl_use_material(material);
        blit(&source.texture, half);
        gl_use_default_material();
    }
}

fn set_target_camera(target: &RenderTarget) {
    let size = target.texture.size();
    let mut camera = Camera2D::from_display_rect(Rect::new(0., 0., size.x, size.y));
    camera.render_target = Some(target.clone());
    set_camera(&camera);
}
fn blit(source: &Texture2D, size: Vec2) {
    draw_texture_ex(
        source,
        0.,
        0.,
        WHITE,
        DrawTextureParams {
            dest_size: Some(size),
            flip_y: true,
            ..Default::default()
        },
    );
}

/// Contract for eventual native adapters. These names intentionally contain no
/// Macroquad types: WinUI can map to Composition brushes and GTK4 to snapshot
/// nodes/shaders while retaining the same scene semantics.
pub trait NativeGlassRenderer {
    fn render_scene(&mut self, scene: &GlassScene);
}
pub struct WinUiGlassRenderer;
pub struct Gtk4GlassRenderer;
impl NativeGlassRenderer for WinUiGlassRenderer {
    fn render_scene(&mut self, _: &GlassScene) { /* Composition-backed adapter lives behind the Windows UI crate. */
    }
}
impl NativeGlassRenderer for Gtk4GlassRenderer {
    fn render_scene(&mut self, _: &GlassScene) { /* GTK snapshot/shader adapter lives behind the GTK UI crate. */
    }
}
