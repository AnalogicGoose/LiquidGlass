//! Backend implementations. The core scene/material vocabulary in
//! [`crate::glass`] knows nothing about any of these; a backend only turns a
//! [`crate::glass::GlassScene`] into GPU commands.

pub mod gl;
