//! SparkGlass library surface.
//!
//! `glass` is the platform-neutral scene vocabulary (Phase 1 semantics).
//! `renderer` is the Macroquad reference renderer, preserved as the visual
//! baseline (Phase 0). `backend::gl` is the windowless `glow`-based renderer
//! that reproduces the same shaders without depending on Macroquad (Phase 3).

pub mod backend;
pub mod glass;
pub mod renderer;
