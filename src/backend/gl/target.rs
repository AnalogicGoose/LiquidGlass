//! Framebuffer + backing-texture pairs used as intermediate render targets.

use glow::HasContext;

pub struct GlTarget {
    pub fbo: glow::NativeFramebuffer,
    pub texture: glow::NativeTexture,
    pub width: i32,
    pub height: i32,
}

impl GlTarget {
    /// Creates an RGBA8 color target. `width`/`height` are clamped to at
    /// least 1 so a minimized/zero-sized host window never produces an
    /// invalid framebuffer.
    pub unsafe fn new(gl: &glow::Context, width: i32, height: i32) -> Self {
        unsafe {
            let width = width.max(1);
            let height = height.max(1);

            let texture = gl.create_texture().expect("create render target texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            let fbo = gl.create_framebuffer().expect("create framebuffer");
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            assert_eq!(
                status,
                glow::FRAMEBUFFER_COMPLETE,
                "SparkGlass GL target incomplete: 0x{status:x}"
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_texture(glow::TEXTURE_2D, None);

            Self {
                fbo,
                texture,
                width,
                height,
            }
        }
    }

    pub unsafe fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.fbo);
            gl.delete_texture(self.texture);
        }
    }

    /// Binds this target and sets the viewport to its full extent. Does not
    /// clear — callers decide whether a clear is needed for the pass.
    pub unsafe fn bind(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            gl.viewport(0, 0, self.width, self.height);
        }
    }
}
