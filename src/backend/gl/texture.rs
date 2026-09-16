//! Plain (non-render-target) texture upload.
//!
//! Per the architecture's CPU-upload rule: this is fine for static,
//! infrequently changing content (background artwork loaded once at
//! startup). It must not run every frame — that would be the GPU->CPU->GPU
//! bounce the architecture forbids in the normal render path.

use glow::HasContext;

pub unsafe fn upload_rgba8(
    gl: &glow::Context,
    width: i32,
    height: i32,
    pixels: &[u8],
) -> glow::NativeTexture {
    unsafe {
        let texture = gl.create_texture().expect("create texture");
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
            Some(pixels),
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
        gl.bind_texture(glow::TEXTURE_2D, None);
        texture
    }
}
