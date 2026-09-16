//! The opaque context handle. Per the FROZEN context-ownership rule: the host
//! creates and owns the GL/GLES context and makes it current on the calling
//! thread *before* calling `lg_create`. SparkGlass never creates a context,
//! window, or swapchain of its own.

use std::ffi::{CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use super::error::LGResult;
use crate::backend::gl::GlGlassRenderer;

/// Resolves a GL function name to its address, exactly like
/// `eglGetProcAddress`/`wglGetProcAddress`/`glXGetProcAddress`. The host
/// supplies this so SparkGlass never has to know which loader is behind it.
pub type LGGlProc = unsafe extern "C" fn(name: *const c_char) -> *const c_void;

/// Opaque handle. The C ABI must never assume anything about this type's
/// layout (FROZEN, §30) — it only ever holds a pointer to it.
pub struct LiquidGlassContext {
    pub(crate) gl: glow::Context,
    pub(crate) renderer: GlGlassRenderer,
    last_error: Option<CString>,
}

impl LiquidGlassContext {
    pub(crate) fn set_error(&mut self, message: impl Into<Vec<u8>>) {
        self.last_error = CString::new(message).ok();
    }
}

/// Creates a context against the GL/GLES context current on the calling
/// thread. Returns null on failure (invalid loader, shader compile failure,
/// or an internal panic — check `lg_last_error` is not meaningful yet at
/// this point since there is no context to attach it to; failures here are
/// reported by returning null only).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lg_create(loader: LGGlProc, width: f32, height: f32) -> *mut LiquidGlassContext {
    if (loader as usize) == 0 {
        return ptr::null_mut();
    }
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let gl = glow::Context::from_loader_function(|name| match CString::new(name) {
            Ok(cname) => loader(cname.as_ptr()),
            Err(_) => ptr::null(),
        });
        let renderer = GlGlassRenderer::new(&gl, width, height);
        Box::new(LiquidGlassContext {
            gl,
            renderer,
            last_error: None,
        })
    }));
    match result {
        Ok(ctx) => Box::into_raw(ctx),
        Err(_) => ptr::null_mut(),
    }
}

/// Destroys a context created by `lg_create`. Passing null is a no-op. GL
/// resource teardown (§29) is still an OPEN question in the architecture
/// doc; today this only frees the Rust-side state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lg_destroy(ctx: *mut LiquidGlassContext) {
    if ctx.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(ctx));
    }));
}

/// Must be called with the same GL context current that was current at
/// `lg_create` time (or an equivalent one sharing its resources).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lg_resize(ctx: *mut LiquidGlassContext, width: f32, height: f32) -> LGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return LGResult::ErrorNullPointer;
    };
    match catch_unwind(AssertUnwindSafe(|| {
        ctx.renderer.resize_if_needed(&ctx.gl, width, height);
    })) {
        Ok(()) => LGResult::Ok,
        Err(_) => LGResult::ErrorPanic,
    }
}

/// Imports a host-owned GL texture as the scene backdrop for the next
/// `lg_render_frame` call, cover-fit to the frame like `main.rs`'s
/// `draw_cover`. The host retains ownership — SparkGlass samples it but
/// never destroys it (FROZEN external-texture-ownership rule, §22).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lg_set_backdrop_gl_texture(
    ctx: *mut LiquidGlassContext,
    gl_texture_id: u32,
    width: f32,
    height: f32,
) -> LGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return LGResult::ErrorNullPointer;
    };
    let Some(name) = std::num::NonZeroU32::new(gl_texture_id) else {
        ctx.set_error("lg_set_backdrop_gl_texture: gl_texture_id must be non-zero");
        return LGResult::ErrorInvalidTexture;
    };
    let texture = glow::NativeTexture(name);
    match catch_unwind(AssertUnwindSafe(|| {
        ctx.renderer.draw_backdrop(&ctx.gl, texture, glam::vec2(width, height));
    })) {
        Ok(()) => LGResult::Ok,
        Err(_) => LGResult::ErrorPanic,
    }
}

/// Draws the last frame rendered by `lg_render_frame` into whichever
/// framebuffer is currently bound. Render-target ownership is still an OPEN
/// question in the architecture doc (§24) — this implements option A
/// ("current framebuffer") as the simplest starting contract; the host binds
/// its target and picks the viewport before calling this.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lg_present(ctx: *mut LiquidGlassContext, dst_width: i32, dst_height: i32) -> LGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return LGResult::ErrorNullPointer;
    };
    match catch_unwind(AssertUnwindSafe(|| {
        ctx.renderer.present(&ctx.gl, dst_width, dst_height);
    })) {
        Ok(()) => LGResult::Ok,
        Err(_) => LGResult::ErrorPanic,
    }
}

/// Returns the last diagnostic message set on this context, or null if none
/// has been set. Valid until the next call on this context; the host must
/// copy it out if it needs to keep it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lg_last_error(ctx: *mut LiquidGlassContext) -> *const c_char {
    let Some(ctx) = (unsafe { ctx.as_ref() }) else {
        return ptr::null();
    };
    ctx.last_error.as_ref().map_or(ptr::null(), |s| s.as_ptr())
}
