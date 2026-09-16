//! Texture import. Per §22 of the architecture doc, the public/FFI surface
//! must not depend directly on `GLuint`:
//!
//! ```text
//! GLuint / host GL texture
//!           ↓
//! sg_import_gl_texture(...)
//!           ↓
//! SGTextureHandle
//!           ↓
//! scene/material references SGTextureHandle
//! ```
//!
//! `SGTextureHandle` is an opaque `u64` (`0` is always invalid, matching the
//! usual C null-handle convention) so a future backend can hand out handles
//! backed by something other than a `glow::NativeTexture` without changing
//! this signature.

use std::collections::HashMap;

use glam::Vec2;

use super::context::SparkGlassContext;

pub type SGTextureHandle = u64;

/// Per-context registry mapping opaque handles to imported GL textures. The
/// host still owns every texture in here — this only ever forgets a
/// reference on release, never calls `glDeleteTextures` (FROZEN
/// external-texture-ownership rule, §22).
pub(crate) struct TextureRegistry {
    next_id: u64,
    textures: HashMap<u64, (glow::NativeTexture, Vec2)>,
}

impl TextureRegistry {
    pub(crate) fn new() -> Self {
        Self {
            next_id: 1,
            textures: HashMap::new(),
        }
    }

    fn insert(&mut self, texture: glow::NativeTexture, size: Vec2) -> SGTextureHandle {
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, (texture, size));
        id
    }

    pub(crate) fn get(&self, handle: SGTextureHandle) -> Option<(glow::NativeTexture, Vec2)> {
        self.textures.get(&handle).copied()
    }

    fn remove(&mut self, handle: SGTextureHandle) {
        self.textures.remove(&handle);
    }
}

/// Imports a host-owned GL texture, returning an opaque handle the scene can
/// reference (see `sg_set_backdrop`). Returns `0` (always invalid) if `ctx`
/// is null or `gl_texture_id` is `0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_import_gl_texture(ctx: *mut SparkGlassContext, gl_texture_id: u32, width: f32, height: f32) -> SGTextureHandle {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return 0;
    };
    let Some(name) = std::num::NonZeroU32::new(gl_texture_id) else {
        ctx.set_error("sg_import_gl_texture: gl_texture_id must be non-zero");
        return 0;
    };
    let texture = glow::NativeTexture(name);
    ctx.textures_mut().insert(texture, Vec2::new(width, height))
}

/// Forgets a handle returned by `sg_import_gl_texture`. Does not destroy the
/// underlying GL texture — the host still owns it. A handle already in use
/// as the current backdrop remains valid for rendering until the context is
/// resized or a new backdrop is set; releasing it only stops future
/// `sg_set_backdrop` calls from being able to look it up.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_release_texture(ctx: *mut SparkGlassContext, handle: SGTextureHandle) {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return;
    };
    ctx.textures_mut().remove(handle);
}
