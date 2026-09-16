//! `glow`-based GL/GLES backend. The host creates and owns the GL context
//! (see `examples/sandbox.rs`); this backend only ever uses the context that
//! is current, per the FROZEN context-ownership rule in the architecture doc.

mod renderer;
mod shader;
mod target;
mod texture;

pub use renderer::GlGlassRenderer;
pub use texture::upload_rgba8;
