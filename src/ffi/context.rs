//! The opaque context handle. Per the FROZEN context-ownership rule: the host
//! creates and owns the GL/GLES context and makes it current on the calling
//! thread *before* calling `sg_create`. SparkGlass never creates a context,
//! window, or swapchain of its own.

use std::ffi::{CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use super::error::SGResult;
use super::texture::{SGTextureHandle, TextureRegistry};
use crate::backend::gl::GlGlassRenderer;

/// Resolves a GL function name to its address, exactly like
/// `eglGetProcAddress`/`wglGetProcAddress`/`glXGetProcAddress`. The host
/// supplies this so SparkGlass never has to know which loader is behind it.
pub type SGGlProc = unsafe extern "C" fn(name: *const c_char) -> *const c_void;

/// Opaque handle. The C ABI must never assume anything about this type's
/// layout (FROZEN, §30) — it only ever holds a pointer to it.
pub struct SparkGlassContext {
    pub(crate) gl: glow::Context,
    pub(crate) renderer: GlGlassRenderer,
    textures: TextureRegistry,
    last_error: Option<CString>,
}

impl SparkGlassContext {
    pub(crate) fn set_error(&mut self, message: impl Into<Vec<u8>>) {
        self.last_error = CString::new(message).ok();
    }

    pub(crate) fn textures_mut(&mut self) -> &mut TextureRegistry {
        &mut self.textures
    }
}

/// Creates a context against the GL/GLES context current on the calling
/// thread. Returns null on failure (invalid loader, shader compile failure,
/// or an internal panic — check `sg_last_error` is not meaningful yet at
/// this point since there is no context to attach it to; failures here are
/// reported by returning null only).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_create(loader: SGGlProc, width: f32, height: f32) -> *mut SparkGlassContext {
    if (loader as usize) == 0 {
        return ptr::null_mut();
    }
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let gl = glow::Context::from_loader_function(|name| match CString::new(name) {
            Ok(cname) => loader(cname.as_ptr()),
            Err(_) => ptr::null(),
        });
        let renderer = GlGlassRenderer::new(&gl, width, height);
        Box::new(SparkGlassContext {
            gl,
            renderer,
            textures: TextureRegistry::new(),
            last_error: None,
        })
    }));
    match result {
        Ok(ctx) => Box::into_raw(ctx),
        Err(_) => ptr::null_mut(),
    }
}

/// Destroys a context created by `sg_create`. Passing null is a no-op. GL
/// resource teardown (§29) is still an OPEN question in the architecture
/// doc; today this only frees the Rust-side state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_destroy(ctx: *mut SparkGlassContext) {
    if ctx.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(ctx));
    }));
}

/// Must be called with the same GL context current that was current at
/// `sg_create` time (or an equivalent one sharing its resources).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_resize(ctx: *mut SparkGlassContext, width: f32, height: f32) -> SGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return SGResult::ErrorNullPointer;
    };
    match catch_unwind(AssertUnwindSafe(|| {
        ctx.renderer.resize_if_needed(&ctx.gl, width, height);
    })) {
        Ok(()) => SGResult::Ok,
        Err(_) => SGResult::ErrorPanic,
    }
}

/// Draws a previously imported texture (see `sg_import_gl_texture`) as the
/// scene backdrop for the next `sg_render_frame` call, cover-fit to the
/// frame like `main.rs`'s `draw_cover`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_set_backdrop(ctx: *mut SparkGlassContext, texture: SGTextureHandle) -> SGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return SGResult::ErrorNullPointer;
    };
    let Some((native, size)) = ctx.textures_mut().get(texture) else {
        ctx.set_error("sg_set_backdrop: unknown texture handle (call sg_import_gl_texture first)");
        return SGResult::ErrorInvalidTexture;
    };
    match catch_unwind(AssertUnwindSafe(|| {
        ctx.renderer.draw_backdrop(&ctx.gl, native, size);
    })) {
        Ok(()) => SGResult::Ok,
        Err(_) => SGResult::ErrorPanic,
    }
}

/// Draws the last frame rendered by `sg_render_frame` into whichever
/// framebuffer is currently bound. Render-target ownership is still an OPEN
/// question in the architecture doc (§24) — this implements option A
/// ("current framebuffer") as the simplest starting contract; the host binds
/// its target and picks the viewport before calling this.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_present(ctx: *mut SparkGlassContext, dst_width: i32, dst_height: i32) -> SGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return SGResult::ErrorNullPointer;
    };
    match catch_unwind(AssertUnwindSafe(|| {
        ctx.renderer.present(&ctx.gl, dst_width, dst_height);
    })) {
        Ok(()) => SGResult::Ok,
        Err(_) => SGResult::ErrorPanic,
    }
}

/// Returns the last diagnostic message set on this context, or null if none
/// has been set. Valid until the next call on this context; the host must
/// copy it out if it needs to keep it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_last_error(ctx: *mut SparkGlassContext) -> *const c_char {
    let Some(ctx) = (unsafe { ctx.as_ref() }) else {
        return ptr::null();
    };
    ctx.last_error.as_ref().map_or(ptr::null(), |s| s.as_ptr())
}
