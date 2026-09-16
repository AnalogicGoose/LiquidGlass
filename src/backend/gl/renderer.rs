//! Windowless `glow` reproduction of `MacroquadGlassRenderer`.
//!
//! This is the Phase 3 decoupling step from
//! `docs/SparkGlass_MASTER_ARCHITECTURE.md`: the same `glass.frag` /
//! `glass_mask.frag` / `blur.frag` fragment shaders, the same multi-pass
//! ping-pong accumulation, driven directly through GL instead of Macroquad.
//! Only the vertex stage and the host-facing plumbing differ — those are
//! pipeline mechanics, not material behavior.
//!
//! The GL context is created and owned by the host (see `examples/sandbox.rs`
//! for a `winit` + `glutin` host); this module only ever uses the context
//! that is current, per the FROZEN context-ownership rule.

use super::shader::compile_program;
use super::target::GlTarget;
use crate::glass::{GlassQuality, GlassScene, GlassSurface};
use glam::Vec2;
use glow::HasContext;

/// Matches `renderer::SHADOW_MARGIN` in the Macroquad reference renderer:
/// glass surfaces draw an oversized quad so the drop shadow and outline,
/// which extend outside the shape's own bounds, are not clipped.
pub const SHADOW_MARGIN: f32 = 96.0;

const VERTEX_SHADER: &str = r#"#version 100
precision highp float;
attribute vec2 a_uv;
varying vec2 v_px;
varying vec2 v_uv;
uniform vec2 u_ndc_size;
uniform vec4 u_rect;
void main() {
    vec2 pos = u_rect.xy + a_uv * u_rect.zw;
    v_px = pos;
    v_uv = vec2(a_uv.x, 1.0 - a_uv.y);
    vec2 ndc = (pos / u_ndc_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

type Loc = Option<glow::NativeUniformLocation>;

unsafe fn loc(gl: &glow::Context, program: glow::NativeProgram, name: &str) -> Loc {
    unsafe { gl.get_uniform_location(program, name) }
}

struct GlassProgram {
    program: glow::NativeProgram,
    ndc_size: Loc,
    rect: Loc,
    resolution: Loc,
    center: Loc,
    size: Loc,
    radius: Loc,
    smoothing: Loc,
    refraction: Loc,
    depth: Loc,
    dispersion: Loc,
    frost: Loc,
    light_intensity: Loc,
    light_angle: Loc,
    splay: Loc,
    tint_mode: Loc,
    tint: Loc,
    shadow: Loc,
    u_scene: Loc,
    u_scene_blur: Loc,
    u_stack_mask: Loc,
}

impl GlassProgram {
    unsafe fn new(gl: &glow::Context) -> Self {
        unsafe {
            let program = compile_program(gl, VERTEX_SHADER, include_str!("../../glass.frag"))
                .expect("glass.frag must compile");
            Self {
                ndc_size: loc(gl, program, "u_ndc_size"),
                rect: loc(gl, program, "u_rect"),
                resolution: loc(gl, program, "u_resolution"),
                center: loc(gl, program, "u_center"),
                size: loc(gl, program, "u_size"),
                radius: loc(gl, program, "u_radius"),
                smoothing: loc(gl, program, "u_smoothing"),
                refraction: loc(gl, program, "u_refraction"),
                depth: loc(gl, program, "u_depth"),
                dispersion: loc(gl, program, "u_dispersion"),
                frost: loc(gl, program, "u_frost"),
                light_intensity: loc(gl, program, "u_light_intensity"),
                light_angle: loc(gl, program, "u_light_angle"),
                splay: loc(gl, program, "u_splay"),
                tint_mode: loc(gl, program, "u_tint_mode"),
                tint: loc(gl, program, "u_tint"),
                shadow: loc(gl, program, "u_shadow"),
                u_scene: loc(gl, program, "u_scene"),
                u_scene_blur: loc(gl, program, "u_scene_blur"),
                u_stack_mask: loc(gl, program, "u_stack_mask"),
                program,
            }
        }
    }
}

struct MaskProgram {
    program: glow::NativeProgram,
    ndc_size: Loc,
    rect: Loc,
    center: Loc,
    size: Loc,
    radius: Loc,
    smoothing: Loc,
}

impl MaskProgram {
    unsafe fn new(gl: &glow::Context) -> Self {
        unsafe {
            let program = compile_program(gl, VERTEX_SHADER, include_str!("../../glass_mask.frag"))
                .expect("glass_mask.frag must compile");
            Self {
                ndc_size: loc(gl, program, "u_ndc_size"),
                rect: loc(gl, program, "u_rect"),
                center: loc(gl, program, "u_center"),
                size: loc(gl, program, "u_size"),
                radius: loc(gl, program, "u_radius"),
                smoothing: loc(gl, program, "u_smoothing"),
                program,
            }
        }
    }
}

struct BlurProgram {
    program: glow::NativeProgram,
    ndc_size: Loc,
    rect: Loc,
    texel: Loc,
    direction: Loc,
    sigma: Loc,
    texture: Loc,
}

impl BlurProgram {
    unsafe fn new(gl: &glow::Context) -> Self {
        unsafe {
            let program = compile_program(gl, VERTEX_SHADER, include_str!("../../blur.frag"))
                .expect("blur.frag must compile");
            Self {
                ndc_size: loc(gl, program, "u_ndc_size"),
                rect: loc(gl, program, "u_rect"),
                texel: loc(gl, program, "u_texel"),
                direction: loc(gl, program, "u_direction"),
                sigma: loc(gl, program, "u_sigma"),
                texture: loc(gl, program, "Texture"),
                program,
            }
        }
    }
}

/// Mirrors `renderer::SceneTargets`: the same nine intermediate targets, for
/// the same reason (see comments there). `output` is additional — a stable
/// handle to the composited result regardless of which ping-pong target the
/// last surface happened to land on.
struct SceneTargets {
    sharp: GlTarget,
    blur: GlTarget,
    blur_tmp: GlTarget,
    stack_a: GlTarget,
    stack_b: GlTarget,
    mask_a: GlTarget,
    mask_b: GlTarget,
    mask_blur: GlTarget,
    mask_blur_tmp: GlTarget,
    output: GlTarget,
}

impl SceneTargets {
    unsafe fn new(gl: &glow::Context, width: i32, height: i32) -> Self {
        unsafe {
            let (hw, hh) = ((width / 2).max(1), (height / 2).max(1));
            Self {
                sharp: GlTarget::new(gl, width, height),
                blur: GlTarget::new(gl, hw, hh),
                blur_tmp: GlTarget::new(gl, hw, hh),
                stack_a: GlTarget::new(gl, width, height),
                stack_b: GlTarget::new(gl, width, height),
                mask_a: GlTarget::new(gl, width, height),
                mask_b: GlTarget::new(gl, width, height),
                mask_blur: GlTarget::new(gl, hw, hh),
                mask_blur_tmp: GlTarget::new(gl, hw, hh),
                output: GlTarget::new(gl, width, height),
            }
        }
    }

    unsafe fn destroy(&self, gl: &glow::Context) {
        unsafe {
            for t in [
                &self.sharp,
                &self.blur,
                &self.blur_tmp,
                &self.stack_a,
                &self.stack_b,
                &self.mask_a,
                &self.mask_b,
                &self.mask_blur,
                &self.mask_blur_tmp,
                &self.output,
            ] {
                t.destroy(gl);
            }
        }
    }
}

pub struct GlGlassRenderer {
    glass: GlassProgram,
    mask: MaskProgram,
    blur: BlurProgram,
    quad_vao: glow::NativeVertexArray,
    // Kept for a future explicit-teardown API (GL resource lifecycle is still
    // OPEN per the architecture doc, §29); not read yet.
    #[allow(dead_code)]
    quad_vbo: glow::NativeBuffer,
    targets: SceneTargets,
    width: i32,
    height: i32,
}

impl GlGlassRenderer {
    pub fn new(gl: &glow::Context, width: f32, height: f32) -> Self {
        unsafe {
            let glass = GlassProgram::new(gl);
            let mask = MaskProgram::new(gl);
            let blur = BlurProgram::new(gl);
            let (quad_vao, quad_vbo) = create_unit_quad(gl);
            let (w, h) = (width.max(1.0) as i32, height.max(1.0) as i32);
            Self {
                glass,
                mask,
                blur,
                quad_vao,
                quad_vbo,
                targets: SceneTargets::new(gl, w, h),
                width: w,
                height: h,
            }
        }
    }

    pub fn resize_if_needed(&mut self, gl: &glow::Context, width: f32, height: f32) {
        let (w, h) = (width.max(1.0) as i32, height.max(1.0) as i32);
        if w != self.width || h != self.height {
            unsafe {
                self.targets.destroy(gl);
                self.targets = SceneTargets::new(gl, w, h);
            }
            self.width = w;
            self.height = h;
        }
    }

    /// Draws `background` (a plain uploaded texture, e.g. static artwork)
    /// into the backdrop target, covering it while preserving aspect ratio —
    /// equivalent to `main.rs::draw_cover` in the Macroquad reference.
    pub fn draw_backdrop(&self, gl: &glow::Context, background: glow::NativeTexture, tex_size: Vec2) {
        unsafe {
            let host_framebuffer = current_framebuffer(gl);

            self.targets.sharp.bind(gl);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::BLEND);

            let frame = Vec2::new(self.width as f32, self.height as f32);
            let scale = (frame.x / tex_size.x).max(frame.y / tex_size.y);
            let size = tex_size * scale;
            let origin = (frame - size) * 0.5;

            self.copy(gl, background, (origin.x, origin.y, size.x, size.y), frame);

            // Leave the framebuffer binding exactly as the host left it —
            // this call must not be visible to whatever the host does next.
            bind_raw_framebuffer(gl, host_framebuffer);
        }
    }

    /// Runs the full multi-pass material pipeline over every surface in the
    /// scene. Mirrors `MacroquadGlassRenderer::render` pass-for-pass.
    pub fn render(&mut self, gl: &glow::Context, scene: &GlassScene) {
        unsafe {
            let host_framebuffer = current_framebuffer(gl);
            let frame = Vec2::new(self.width as f32, self.height as f32);

            self.targets.stack_a.bind(gl);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::BLEND);
            self.copy(gl, self.targets.sharp.texture, (0.0, 0.0, frame.x, frame.y), frame);

            self.targets.mask_a.bind(gl);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);

            let mut current = &self.targets.stack_a;
            let mut next = &self.targets.stack_b;
            let mut current_mask = &self.targets.mask_a;
            let mut next_mask = &self.targets.mask_b;

            gl.use_program(Some(self.glass.program));
            gl.uniform_2_f32(self.glass.resolution.as_ref(), frame.x, frame.y);

            for surface in &scene.surfaces {
                let frost = surface_frost(surface, scene.quality, scene.reduced_transparency);

                self.blur_into(
                    gl,
                    current.texture,
                    &self.targets.blur,
                    &self.targets.blur_tmp,
                    frost * 0.5,
                );
                self.blur_into(
                    gl,
                    current_mask.texture,
                    &self.targets.mask_blur,
                    &self.targets.mask_blur_tmp,
                    frost * 0.5,
                );

                next.bind(gl);
                gl.clear_color(0.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.disable(glow::BLEND);
                self.copy(gl, current.texture, (0.0, 0.0, frame.x, frame.y), frame);

                self.draw_surface(gl, surface, scene.quality, scene.reduced_transparency, current.texture, frame);

                next_mask.bind(gl);
                gl.clear_color(0.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.disable(glow::BLEND);
                self.copy(gl, current_mask.texture, (0.0, 0.0, frame.x, frame.y), frame);

                self.draw_mask(gl, surface, frame);

                std::mem::swap(&mut current, &mut next);
                std::mem::swap(&mut current_mask, &mut next_mask);
            }

            self.targets.output.bind(gl);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::BLEND);
            self.copy(gl, current.texture, (0.0, 0.0, frame.x, frame.y), frame);

            // Restore exactly what was bound when this call started. Without
            // this, `present()` — which trusts "whatever is currently bound
            // is the host's target" — would silently draw into our own
            // internal `output` target instead of the host's real
            // framebuffer, since that's what this function leaves bound.
            // (This was a real bug: every example's on-screen output was
            // broken by it even though a same-process glReadPixels capture
            // right after `present()` looked byte-correct, because that
            // capture was reading `output` back, not the host's actual
            // framebuffer.)
            bind_raw_framebuffer(gl, host_framebuffer);
        }
    }

    pub fn output_texture(&self) -> glow::NativeTexture {
        self.targets.output.texture
    }

    /// Draws the composited result into whichever framebuffer is currently
    /// bound (typically the window's default framebuffer, framebuffer 0).
    /// The host binds it and picks the viewport; SparkGlass never assumes it
    /// owns the swapchain.
    pub fn present(&self, gl: &glow::Context, dst_width: i32, dst_height: i32) {
        unsafe {
            gl.viewport(0, 0, dst_width, dst_height);
            gl.disable(glow::BLEND);
            self.copy(
                gl,
                self.targets.output.texture,
                (0.0, 0.0, dst_width as f32, dst_height as f32),
                Vec2::new(dst_width as f32, dst_height as f32),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_surface(
        &self,
        gl: &glow::Context,
        surface: &GlassSurface,
        quality: GlassQuality,
        reduced_transparency: bool,
        scene_tex: glow::NativeTexture,
        frame: Vec2,
    ) {
        unsafe {
            let geometry = surface.geometry;
            let frost = surface_frost(surface, quality, reduced_transparency);
            let refraction = if matches!(quality, GlassQuality::Low | GlassQuality::Fallback) {
                0.0
            } else {
                surface.optics.refraction_strength
            };

            gl.use_program(Some(self.glass.program));
            gl.uniform_2_f32(self.glass.center.as_ref(), geometry.center().x, geometry.center().y);
            gl.uniform_2_f32(self.glass.size.as_ref(), geometry.size().x, geometry.size().y);
            gl.uniform_1_f32(self.glass.radius.as_ref(), geometry.radius());
            gl.uniform_1_f32(self.glass.smoothing.as_ref(), geometry.smoothing());
            gl.uniform_1_f32(self.glass.refraction.as_ref(), refraction);
            gl.uniform_1_f32(self.glass.depth.as_ref(), surface.optics.depth);
            gl.uniform_1_f32(self.glass.dispersion.as_ref(), surface.optics.dispersion);
            gl.uniform_1_f32(self.glass.frost.as_ref(), frost);
            gl.uniform_1_f32(self.glass.light_intensity.as_ref(), surface.lighting.intensity);
            gl.uniform_1_f32(self.glass.light_angle.as_ref(), surface.lighting.angle_degrees);
            gl.uniform_1_f32(self.glass.splay.as_ref(), surface.lighting.splay);
            gl.uniform_1_f32(
                self.glass.tint_mode.as_ref(),
                if surface.material.dark_tint { 1.0 } else { 0.0 },
            );
            gl.uniform_1_f32(self.glass.tint.as_ref(), surface.material.tint_opacity);
            gl.uniform_1_f32(self.glass.shadow.as_ref(), surface.lighting.shadow_strength);
            gl.uniform_2_f32(self.glass.ndc_size.as_ref(), frame.x, frame.y);

            let top_left = geometry.center() - geometry.size() * 0.5 - SHADOW_MARGIN;
            let quad = geometry.size() + SHADOW_MARGIN * 2.0;
            gl.uniform_4_f32(self.glass.rect.as_ref(), top_left.x, top_left.y, quad.x, quad.y);

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(scene_tex));
            gl.uniform_1_i32(self.glass.u_scene.as_ref(), 0);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.targets.blur.texture));
            gl.uniform_1_i32(self.glass.u_scene_blur.as_ref(), 1);
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.targets.mask_blur.texture));
            gl.uniform_1_i32(self.glass.u_stack_mask.as_ref(), 2);

            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.draw_quad(gl, self.quad_vao);
            gl.disable(glow::BLEND);
        }
    }

    unsafe fn draw_mask(&self, gl: &glow::Context, surface: &GlassSurface, frame: Vec2) {
        unsafe {
            let geometry = surface.geometry;
            gl.use_program(Some(self.mask.program));
            gl.uniform_2_f32(self.mask.center.as_ref(), geometry.center().x, geometry.center().y);
            gl.uniform_2_f32(self.mask.size.as_ref(), geometry.size().x, geometry.size().y);
            gl.uniform_1_f32(self.mask.radius.as_ref(), geometry.radius());
            gl.uniform_1_f32(self.mask.smoothing.as_ref(), geometry.smoothing());
            gl.uniform_2_f32(self.mask.ndc_size.as_ref(), frame.x, frame.y);

            let top_left = geometry.center() - geometry.size() * 0.5 - 1.0;
            let quad = geometry.size() + 2.0;
            gl.uniform_4_f32(self.mask.rect.as_ref(), top_left.x, top_left.y, quad.x, quad.y);

            gl.disable(glow::BLEND);
            self.draw_quad(gl, self.quad_vao);
        }
    }

    unsafe fn blur_into(
        &self,
        gl: &glow::Context,
        source: glow::NativeTexture,
        output: &GlTarget,
        scratch: &GlTarget,
        sigma: f32,
    ) {
        unsafe {
            let half = Vec2::new(output.width as f32, output.height as f32);
            output.bind(gl);
            gl.disable(glow::BLEND);
            self.copy(gl, source, (0.0, 0.0, half.x, half.y), half);

            gl.use_program(Some(self.blur.program));
            gl.uniform_2_f32(self.blur.texel.as_ref(), 1.0 / half.x, 1.0 / half.y);
            gl.uniform_1_f32(self.blur.sigma.as_ref(), sigma * 0.5);
            gl.uniform_2_f32(self.blur.ndc_size.as_ref(), half.x, half.y);
            gl.uniform_4_f32(self.blur.rect.as_ref(), 0.0, 0.0, half.x, half.y);

            for (src, dst, direction) in [(output, scratch, (1.0, 0.0)), (scratch, output, (0.0, 1.0))] {
                dst.bind(gl);
                gl.uniform_2_f32(self.blur.direction.as_ref(), direction.0, direction.1);
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(src.texture));
                gl.uniform_1_i32(self.blur.texture.as_ref(), 0);
                self.draw_quad(gl, self.quad_vao);
            }
        }
    }

    /// Generic textured-quad copy, reusing `blur.frag`'s own sigma<0.05
    /// pass-through path — see that shader for why that is a plain copy.
    unsafe fn copy(&self, gl: &glow::Context, source: glow::NativeTexture, rect: (f32, f32, f32, f32), ndc_size: Vec2) {
        unsafe {
            gl.use_program(Some(self.blur.program));
            gl.uniform_1_f32(self.blur.sigma.as_ref(), 0.0);
            gl.uniform_2_f32(self.blur.ndc_size.as_ref(), ndc_size.x, ndc_size.y);
            gl.uniform_4_f32(self.blur.rect.as_ref(), rect.0, rect.1, rect.2, rect.3);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(source));
            gl.uniform_1_i32(self.blur.texture.as_ref(), 0);
            self.draw_quad(gl, self.quad_vao);
        }
    }

    unsafe fn draw_quad(&self, gl: &glow::Context, vao: glow::NativeVertexArray) {
        unsafe {
            gl.bind_vertex_array(Some(vao));
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }
}

/// Reads the currently bound `GL_DRAW_FRAMEBUFFER`. `0` means the default
/// framebuffer (glow represents that as `None`, not `NativeFramebuffer`).
unsafe fn current_framebuffer(gl: &glow::Context) -> i32 {
    unsafe { gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) }
}

/// Inverse of [`current_framebuffer`] — rebinds a raw framebuffer id
/// previously read from it, including the `0` (default framebuffer) case.
unsafe fn bind_raw_framebuffer(gl: &glow::Context, raw: i32) {
    let framebuffer = std::num::NonZeroU32::new(raw as u32).map(glow::NativeFramebuffer);
    unsafe { gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer) };
}

fn surface_frost(surface: &GlassSurface, quality: GlassQuality, reduced_transparency: bool) -> f32 {
    if reduced_transparency || matches!(quality, GlassQuality::Fallback) {
        0.0
    } else {
        surface.material.frost_radius
    }
}

unsafe fn create_unit_quad(gl: &glow::Context) -> (glow::NativeVertexArray, glow::NativeBuffer) {
    unsafe {
        // (0,0)-(1,0)-(0,1)-(1,1): a_uv doubles as the unit-quad corner; the
        // vertex shader positions the quad via `u_rect`.
        const CORNERS: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let vao = gl.create_vertex_array().expect("create vertex array");
        let vbo = gl.create_buffer().expect("create buffer");
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck_cast(&CORNERS), glow::STATIC_DRAW);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
        gl.bind_vertex_array(None);
        (vao, vbo)
    }
}

fn bytemuck_cast(data: &[f32]) -> &[u8] {
    // SAFETY: `f32` has no padding/alignment surprises here; this is a plain
    // reinterpret of a `&[f32]` as bytes for `buffer_data_u8_slice`.
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), std::mem::size_of_val(data)) }
}
