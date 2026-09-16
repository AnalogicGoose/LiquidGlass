//! C ABI boundary — Phase 2 of `docs/SparkGlass_MASTER_ARCHITECTURE.md`.
//!
//! Every exported symbol is `extern "C"`, operates only on ABI-safe POD
//! structs and an opaque context pointer, and never lets a Rust panic
//! unwind across the boundary (FROZEN, §30/§33). A C/C++ host needs no
//! Rust-specific knowledge to call any of this.
//!
//! Typical sequence:
//!
//! ```text
//! sg_create(loader, width, height)
//! sg_import_gl_texture(ctx, gl_texture_id, width, height) -> handle   // once per texture
//! sg_set_backdrop(ctx, handle)                                       // per frame, or once for static art
//! sg_render_frame(ctx, &frame)
//! sg_present(ctx, dst_width, dst_height)                    // with the host's target framebuffer bound
//! sg_resize(ctx, width, height)                              // on resize
//! sg_release_texture(ctx, handle)                            // when the host is done with it
//! sg_destroy(ctx)
//! ```

mod context;
mod error;
mod frame;
mod texture;

pub use context::{SGGlProc, SparkGlassContext, sg_create, sg_destroy, sg_last_error, sg_present, sg_resize, sg_set_backdrop};
pub use error::SGResult;
pub use frame::{SGFrame, SGGlassElement, SGQuality, sg_render_frame};
pub use texture::{SGTextureHandle, sg_import_gl_texture, sg_release_texture};
