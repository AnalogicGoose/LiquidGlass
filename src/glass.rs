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

/// Roadmap Phase 9.5 ("Interaction Illumination"): how strongly a surface's
/// edge should glow for its current `GlassInteraction` state, 0..1. `Idle`
/// is exactly `0.0` — a true no-op, so every surface that's never touched
/// (every visual regression golden, every style-gallery scene) renders
/// unchanged. The ordering (hover < selected < pressed < dragged) is a
/// judgment call, not derived from anything — the roadmap specifies the
/// *mechanism* ("pointer/touch → energy field → local glow → edge
/// response"), not these particular numbers.
pub fn interaction_energy(interaction: GlassInteraction) -> f32 {
    match interaction {
        GlassInteraction::Idle => 0.0,
        GlassInteraction::Hovered => 0.25,
        GlassInteraction::Selected => 0.35,
        GlassInteraction::Pressed => 0.55,
        GlassInteraction::Dragged => 1.0,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlassMaterial {
    pub frost_radius: f32,
    pub tint_opacity: f32,
    pub dark_tint: bool,
    /// Final color-grade stage in `glass.frag`, applied after tint and
    /// before highlights. `1.0` (the `preset()` default) is a no-op — every
    /// shipped style is neutral here unless a product opts in.
    pub saturation: f32,
    /// See `saturation`. `0.0` is a no-op.
    pub brightness: f32,
    /// See `saturation`. `1.0` is a no-op.
    pub contrast: f32,
    /// Roadmap Phase 8.8 "Clear Material Dimming": darkens the refracted
    /// backdrop in proportion to its own local luminance, before highlights
    /// are added — protects legibility of whatever native foreground
    /// content the host draws on top of a high-transmission ("Clear")
    /// surface. `0.0` (the `preset()` default) is a no-op; it's meant to be
    /// opted into per-style, not applied globally.
    pub clear_dimming: f32,
    /// Roadmap Phase 8.5 "Adaptive Material Response": scales how much
    /// extra rim/edge separation `glass.frag` adds in front of a locally
    /// "busy" backdrop (where the sharp and frosted samples disagree the
    /// most) — see 8.7's "busy backgrounds may require stronger
    /// separation". `0.0` (the `preset()` default) is a no-op.
    pub adaptive_response: f32,
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
    /// Wired via `interaction_energy()` to a real shader uniform (Phase
    /// 9.5, "Interaction Illumination") — a surface's edge glows while
    /// it's being interacted with. `main.rs` sets this to `Dragged` while
    /// dragging a panel; `Hovered`/`Pressed`/`Selected` exist in the enum
    /// but nothing currently sets them (no hover/press-vs-click detection
    /// exists in any host yet).
    pub interaction: GlassInteraction,
    pub style: GlassStyle,
}

#[derive(Debug)]
pub struct GlassGroup {
    pub name: &'static str,
    pub surface_ids: Vec<u64>,
    pub style: GlassStyle,
}

/// Roadmap Phase 9.1 ("Explicit Glass Containers"): why a group couldn't be
/// declared or why a lookup couldn't be made — see `GlassScene::add_group`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupError {
    /// `add_group` was called with an empty `surface_ids` — a container
    /// with no members isn't a container.
    EmptyGroup,
    /// Another group already used this name; names are how product code
    /// looks a group back up, so they must be unique within a scene.
    DuplicateName,
    /// `surface_ids` referenced an id that isn't in `GlassScene::surfaces`
    /// at the time `add_group` was called.
    UnknownSurfaceId(u64),
}

impl std::fmt::Display for GroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGroup => write!(f, "a glass group needs at least one surface"),
            Self::DuplicateName => write!(f, "a group with this name already exists in the scene"),
            Self::UnknownSurfaceId(id) => write!(f, "surface id {id} is not in this scene"),
        }
    }
}

impl std::error::Error for GroupError {}

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
            // Starts empty rather than seeded with placeholder data, since
            // fabricated group membership referencing hardcoded surface
            // ids would be wrong for any scene that doesn't happen to
            // match them. Call `add_group` once the real surfaces (and
            // their real ids) are known — see Phase 9.1 below.
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

    /// Roadmap Phase 9.1 ("Explicit Glass Containers"): declares that
    /// `surface_ids` belong to one material region. Membership is by
    /// stable `id`, not vector index, so it survives `bring_to_front`/
    /// `bring_group_to_front` reordering `self.surfaces` — the same reason
    /// `GlassSurface::id` exists at all (see `surface_at`'s hit-testing).
    ///
    /// Every id must already be in `self.surfaces`, and the name must be
    /// unique within this scene — see `GroupError`.
    pub fn add_group(&mut self, name: &'static str, style: GlassStyle, surface_ids: Vec<u64>) -> Result<(), GroupError> {
        if surface_ids.is_empty() {
            return Err(GroupError::EmptyGroup);
        }
        if self.groups.iter().any(|g| g.name == name) {
            return Err(GroupError::DuplicateName);
        }
        for &id in &surface_ids {
            if !self.surfaces.iter().any(|s| s.id == id) {
                return Err(GroupError::UnknownSurfaceId(id));
            }
        }
        self.groups.push(GlassGroup { name, surface_ids, style });
        Ok(())
    }

    pub fn group(&self, name: &str) -> Option<&GlassGroup> {
        self.groups.iter().find(|g| g.name == name)
    }

    /// Every surface currently in `self.surfaces` belonging to group
    /// `name`, in their current z-order. A member id with no matching
    /// surface (removed from the scene after the group was declared) is
    /// silently skipped — group membership tracks intent, not scene
    /// lifetime, so this is not an error case.
    pub fn group_surfaces<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a GlassSurface> + 'a {
        let ids = self.group(name).map(|g| g.surface_ids.clone()).unwrap_or_default();
        self.surfaces.iter().filter(move |s| ids.contains(&s.id))
    }

    /// The group-level equivalent of `bring_to_front`: moves every member
    /// surface to the front together, preserving their relative order, so
    /// e.g. dragging a panel with an attached control pill can move the
    /// whole cluster without the pill getting left behind underneath it.
    /// A no-op if `name` doesn't exist.
    pub fn bring_group_to_front(&mut self, name: &str) {
        let Some(ids) = self.group(name).map(|g| g.surface_ids.clone()) else {
            return;
        };
        let (mut moved, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.surfaces)
            .into_iter()
            .partition(|s| ids.contains(&s.id));
        moved.sort_by_key(|s| ids.iter().position(|&id| id == s.id));
        self.surfaces = rest;
        self.surfaces.extend(moved);
    }

    /// Forces every member surface's `.style` to match the group's —
    /// Phase 9.1's "these elements belong to one material region" as
    /// something enforceable, not just a convention the caller has to
    /// remember by hand on every surface individually. Extends Phase
    /// 8.1's per-style material bucketing (`preset()`) to the container
    /// level. A no-op if `name` doesn't exist.
    pub fn apply_group_style(&mut self, name: &str) {
        let Some((ids, style)) = self.group(name).map(|g| (g.surface_ids.clone(), g.style)) else {
            return;
        };
        for surface in &mut self.surfaces {
            if ids.contains(&surface.id) {
                surface.style = style;
            }
        }
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
            clear_dimming: 0.0,
            adaptive_response: 0.0,
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

    #[test]
    fn every_style_ships_the_phase_8_color_and_adaptive_knobs_as_no_ops() {
        for style in [
            GlassStyle::Thin,
            GlassStyle::Regular,
            GlassStyle::Prominent,
            GlassStyle::Control,
            GlassStyle::Navigation,
        ] {
            let (material, ..) = preset(style, false);
            assert_eq!(material.saturation, 1.0, "{style:?}");
            assert_eq!(material.brightness, 0.0, "{style:?}");
            assert_eq!(material.contrast, 1.0, "{style:?}");
            assert_eq!(material.clear_dimming, 0.0, "{style:?}");
            assert_eq!(material.adaptive_response, 0.0, "{style:?}");
        }
    }

    fn test_surface(id: u64) -> GlassSurface {
        let (material, optics, lighting) = preset(GlassStyle::Regular, false);
        GlassSurface {
            id,
            geometry: GlassGeometry::RoundedRect {
                center: Vec2::ZERO,
                size: vec2(10.0, 10.0),
                radius: 4.0,
                smoothing: 0.0,
            },
            material,
            optics,
            lighting,
            interaction: GlassInteraction::Idle,
            style: GlassStyle::Regular,
        }
    }

    #[test]
    fn add_group_rejects_empty_duplicate_and_unknown_ids() {
        let mut scene = GlassScene::new(vec![test_surface(1), test_surface(2)]);
        assert_eq!(scene.add_group("panel", GlassStyle::Regular, vec![]), Err(GroupError::EmptyGroup));
        assert_eq!(
            scene.add_group("panel", GlassStyle::Regular, vec![99]),
            Err(GroupError::UnknownSurfaceId(99))
        );
        assert_eq!(scene.add_group("panel", GlassStyle::Regular, vec![1, 2]), Ok(()));
        assert_eq!(
            scene.add_group("panel", GlassStyle::Regular, vec![1]),
            Err(GroupError::DuplicateName)
        );
    }

    #[test]
    fn group_surfaces_returns_only_members_and_skips_removed_ones() {
        let mut scene = GlassScene::new(vec![test_surface(1), test_surface(2), test_surface(3)]);
        scene.add_group("cluster", GlassStyle::Regular, vec![1, 3]).unwrap();
        let members: Vec<u64> = scene.group_surfaces("cluster").map(|s| s.id).collect();
        assert_eq!(members, vec![1, 3]);

        // Removing a member surface from the scene (not from the group —
        // membership tracks intent, not scene lifetime) shouldn't panic or
        // resurrect it.
        scene.surfaces.retain(|s| s.id != 3);
        let members: Vec<u64> = scene.group_surfaces("cluster").map(|s| s.id).collect();
        assert_eq!(members, vec![1]);
    }

    #[test]
    fn bring_group_to_front_moves_members_together_preserving_relative_order() {
        let mut scene = GlassScene::new(vec![test_surface(1), test_surface(2), test_surface(3), test_surface(4)]);
        // Group declared as [1, 3]; scene z-order is currently [1, 2, 3, 4].
        scene.add_group("cluster", GlassStyle::Regular, vec![1, 3]).unwrap();
        scene.bring_group_to_front("cluster");
        let order: Vec<u64> = scene.surfaces.iter().map(|s| s.id).collect();
        // Non-members keep their relative order at the back; members move
        // to the front together, in the group's declared order (matching
        // `add_group`'s [1, 3], not whatever order `partition` happened to
        // leave them in).
        assert_eq!(order, vec![2, 4, 1, 3]);
    }

    #[test]
    fn bring_group_to_front_is_a_no_op_for_an_unknown_group() {
        let mut scene = GlassScene::new(vec![test_surface(1), test_surface(2)]);
        scene.bring_group_to_front("does-not-exist");
        let order: Vec<u64> = scene.surfaces.iter().map(|s| s.id).collect();
        assert_eq!(order, vec![1, 2]);
    }

    #[test]
    fn apply_group_style_only_touches_members() {
        let mut scene = GlassScene::new(vec![test_surface(1), test_surface(2)]);
        scene.add_group("pill", GlassStyle::Control, vec![1]).unwrap();
        scene.apply_group_style("pill");
        assert_eq!(scene.surfaces[0].style, GlassStyle::Control);
        assert_eq!(scene.surfaces[1].style, GlassStyle::Regular);
    }

    #[test]
    fn interaction_energy_is_a_true_no_op_at_idle_and_increases_toward_dragged() {
        assert_eq!(interaction_energy(GlassInteraction::Idle), 0.0);
        let energies = [
            interaction_energy(GlassInteraction::Hovered),
            interaction_energy(GlassInteraction::Selected),
            interaction_energy(GlassInteraction::Pressed),
            interaction_energy(GlassInteraction::Dragged),
        ];
        for pair in energies.windows(2) {
            assert!(pair[0] < pair[1], "{energies:?} should be strictly increasing");
        }
        assert_eq!(interaction_energy(GlassInteraction::Dragged), 1.0);
    }
}
