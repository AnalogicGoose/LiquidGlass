//! Platform-neutral SparkGlass vocabulary.
//!
//! This module deliberately has no knowledge of Macroquad, WinUI, GTK, or a
//! GPU API.  A product UI requests a semantic `GlassStyle`; a platform
//! renderer maps it to the native compositor/effect pipeline it supports.
#![allow(dead_code)] // Public semantic vocabulary is consumed by future UI backends.

use glam::{Vec2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassStyle {
    Thin,
    Regular,
    Prominent,
    Control,
    Navigation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassQuality {
    Ultra,
    High,
    Medium,
    Low,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassInteraction {
    Idle,
    Hovered,
    Pressed,
    Dragged,
    Selected,
}

#[derive(Clone, Copy, Debug)]
pub struct GlassMaterial {
    pub frost_radius: f32,
    pub tint_opacity: f32,
    pub dark_tint: bool,
    /// Not yet wired to any shader uniform — `glass.frag` has no color-grade
    /// stage. Setting this has no visual effect today; it's a placeholder
    /// for that stage if one gets added (Phase 8's "adaptive tint/luminance"
    /// work is the likely place). Verified by grep: every write-site sets a
    /// constant, no read-site exists anywhere in either renderer.
    pub saturation: f32,
    /// Not yet wired to any shader uniform — same situation as `saturation`.
    pub brightness: f32,
    /// Not yet wired to any shader uniform — same situation as `saturation`.
    pub contrast: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GlassOptics {
    pub refraction_strength: f32,
    pub depth: f32,
    pub dispersion: f32,
    /// Not yet wired to any shader uniform — `glass.frag`'s SDF-based edge
    /// curvature is currently derived entirely from `depth`/`smoothing`,
    /// not a separate curvature input. Setting this has no visual effect
    /// today. Verified by grep: every write-site sets a constant (0.6), no
    /// read-site exists anywhere in either renderer.
    pub surface_curvature: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GlassLighting {
    pub intensity: f32,
    pub angle_degrees: f32,
    pub splay: f32,
    pub shadow_strength: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum GlassGeometry {
    RoundedRect {
        center: Vec2,
        size: Vec2,
        radius: f32,
        smoothing: f32,
    },
}

impl GlassGeometry {
    pub fn center(self) -> Vec2 {
        match self {
            Self::RoundedRect { center, .. } => center,
        }
    }
    pub fn size(self) -> Vec2 {
        match self {
            Self::RoundedRect { size, .. } => size,
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Self::RoundedRect { radius, .. } => radius,
        }
    }
    pub fn smoothing(self) -> f32 {
        match self {
            Self::RoundedRect { smoothing, .. } => smoothing,
        }
    }

    /// CPU hit testing mirrors the rounded rectangle used by the renderer.
    pub fn contains(self, point: Vec2) -> bool {
        let half = self.size() * 0.5;
        let radius = self.radius().min(half.x).min(half.y);
        let q = (point - self.center()).abs() - half + vec2(radius, radius);
        q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - radius <= 0.0
    }

    pub fn with_center(self, center: Vec2) -> Self {
        match self {
            Self::RoundedRect {
                size,
                radius,
                smoothing,
                ..
            } => Self::RoundedRect {
                center,
                size,
                radius,
                smoothing,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlassSurface {
    pub id: u64,
    pub geometry: GlassGeometry,
    pub material: GlassMaterial,
    pub optics: GlassOptics,
    pub lighting: GlassLighting,
    /// Tracked (`main.rs` sets it while dragging a panel) but not yet wired
    /// to any shader uniform — no renderer reads it. This one has a clearer
    /// future than `GlassMaterial`'s dead fields: it's what roadmap Phase
    /// 9.5 ("Interaction Illumination") would consume.
    pub interaction: GlassInteraction,
    pub style: GlassStyle,
}

#[derive(Debug)]
pub struct GlassGroup {
    pub name: &'static str,
    pub surface_ids: Vec<u64>,
    pub style: GlassStyle,
}

/// Per-window shared rendering state. Its backdrop is rendered once by the
/// renderer, then all surfaces sample it in this common coordinate system.
#[derive(Debug)]
pub struct GlassScene {
    pub surfaces: Vec<GlassSurface>,
    pub groups: Vec<GlassGroup>,
    pub quality: GlassQuality,
    /// Genuinely wired: both renderers zero a surface's frost when this is
    /// set (see `surface_frost()` in `src/renderer.rs` and
    /// `src/backend/gl/renderer.rs`).
    pub reduced_transparency: bool,
    /// Accepted (including from the FFI frame struct) but not yet
    /// consumed by any renderer — there's no animation/motion system yet
    /// for it to reduce (roadmap Phase 9.4 "Morphing").
    pub reduced_motion: bool,
    pub frame: u64,
}

impl GlassScene {
    pub fn new(surfaces: Vec<GlassSurface>) -> Self {
        Self {
            surfaces,
            // Container/group semantics are FUTURE work (architecture doc
            // §13) — nothing reads `groups` yet. Left empty rather than
            // seeded with placeholder data, since fabricated group
            // membership referencing hardcoded surface ids would be wrong
            // for any scene that doesn't happen to match them.
            groups: Vec::new(),
            quality: GlassQuality::High,
            reduced_transparency: false,
            reduced_motion: false,
            frame: 0,
        }
    }

    pub fn surface_at(&self, point: Vec2) -> Option<usize> {
        self.surfaces
            .iter()
            .rposition(|surface| surface.geometry.contains(point))
    }

    pub fn bring_to_front(&mut self, index: usize) {
        let surface = self.surfaces.remove(index);
        self.surfaces.push(surface);
    }
}

/// Sane semantic defaults. Product code should choose these rather than pass
/// shader constants around.
///
/// Frost/tint are bucketed by size class, matching the Figma reference's own
/// `Frost - Regular` (6, small pill-sized controls) vs `Frost - Large` (16,
/// full panels) split — see `docs/references/figma-liquid-glass/README.md`.
/// `refraction_strength`/`depth`/`dispersion`/lighting stay flat across
/// styles below because that also matches the reference: those values were
/// identical across every instance checked there, regardless of size or
/// light/dark variant.
pub fn preset(style: GlassStyle, dark_tint: bool) -> (GlassMaterial, GlassOptics, GlassLighting) {
    let (frost_radius, tint_opacity) = match style {
        GlassStyle::Thin | GlassStyle::Control => (6.0, 0.15),
        GlassStyle::Regular | GlassStyle::Navigation => (16.0, 1.0),
        GlassStyle::Prominent => (22.0, 1.0),
    };
    (
        GlassMaterial {
            frost_radius,
            tint_opacity,
            dark_tint,
            saturation: 1.0,
            brightness: 0.0,
            contrast: 1.0,
        },
        GlassOptics {
            refraction_strength: 2.0,
            depth: 30.0,
            dispersion: 0.2,
            surface_curvature: 0.6,
        },
        GlassLighting {
            intensity: 0.25,
            angle_degrees: 0.0,
            splay: 0.2,
            shadow_strength: 1.0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_geometry_uses_scene_coordinates_for_hit_testing() {
        let geometry = GlassGeometry::RoundedRect {
            center: vec2(100.0, 100.0),
            size: vec2(80.0, 40.0),
            radius: 20.0,
            smoothing: 0.0,
        };
        assert!(geometry.contains(vec2(100.0, 100.0)));
        assert!(!geometry.contains(vec2(20.0, 100.0)));
    }

    #[test]
    fn thin_preset_preserves_the_clear_material_values() {
        let (material, optics, lighting) = preset(GlassStyle::Thin, false);
        assert_eq!(material.frost_radius, 6.0);
        assert_eq!(material.tint_opacity, 0.15);
        assert_eq!(optics.refraction_strength, 2.0);
        assert_eq!(lighting.intensity, 0.25);
    }
}
