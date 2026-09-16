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
//! lg_create(loader, width, height)
//! lg_set_backdrop_gl_texture(ctx, texture, width, height)   // per frame, or once for static art
//! lg_render_frame(ctx, &frame)
//! lg_present(ctx, dst_width, dst_height)                    // with the host's target framebuffer bound
//! lg_resize(ctx, width, height)                              // on resize
//! lg_destroy(ctx)
//! ```

mod context;
mod error;
mod frame;

pub use context::{LGGlProc, LiquidGlassContext, lg_create, lg_destroy, lg_last_error, lg_present, lg_resize, lg_set_backdrop_gl_texture};
pub use error::LGResult;
pub use frame::{LGFrame, LGGlassElement, LGQuality, lg_render_frame};
