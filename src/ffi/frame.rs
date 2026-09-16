//! Frame submission. Per the FROZEN "host describes intent, SparkGlass
//! decides the render graph" principle: the host fills one `SGFrame`
//! describing every glass element for this frame and calls
//! `sg_render_frame` once — it never drives mask/blur/composite passes
//! itself.

use std::panic::{AssertUnwindSafe, catch_unwind};

use glam::vec2;

use super::context::SparkGlassContext;
use super::error::SGResult;
use crate::glass::{
    GlassGeometry, GlassInteraction, GlassLighting, GlassMaterial, GlassOptics, GlassQuality, GlassScene, GlassStyle,
    GlassSurface,
};

/// Mirrors [`GlassQuality`] with a stable, explicit integer representation
/// for the ABI (Rust enum layouts are not guaranteed stable across the
/// boundary — FROZEN, §30).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SGQuality {
    Ultra = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Fallback = 4,
}

impl From<SGQuality> for GlassQuality {
    fn from(quality: SGQuality) -> Self {
        match quality {
            SGQuality::Ultra => GlassQuality::Ultra,
            SGQuality::High => GlassQuality::High,
            SGQuality::Medium => GlassQuality::Medium,
            SGQuality::Low => GlassQuality::Low,
            SGQuality::Fallback => GlassQuality::Fallback,
        }
    }
}

/// One glass surface. Field names mirror [`GlassSurface`]'s semantic
/// properties directly (per §34: expose material meaning, not shader
/// uniforms) rather than the current GLSL uniform names.
///
/// `GlassGeometry` only has one variant (`RoundedRect`) today, so this
/// struct is that variant flattened; a `shape_kind` tag can be added the day
/// a second geometry kind exists without breaking this layout (new fields
/// only ever append).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SGGlassElement {
    pub id: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub size_x: f32,
    pub size_y: f32,
    pub radius: f32,
    pub smoothing: f32,
    pub refraction_strength: f32,
    pub depth: f32,
    pub dispersion: f32,
    pub frost_radius: f32,
    pub light_intensity: f32,
    pub light_angle_degrees: f32,
    pub light_splay: f32,
    pub tint_opacity: f32,
    /// 0 = light tint, non-zero = dark tint.
    pub dark_tint: u8,
    pub shadow_strength: f32,
}

fn element_to_surface(element: &SGGlassElement) -> GlassSurface {
    GlassSurface {
        id: element.id,
        geometry: GlassGeometry::RoundedRect {
            center: vec2(element.center_x, element.center_y),
            size: vec2(element.size_x, element.size_y),
            radius: element.radius,
            smoothing: element.smoothing,
        },
        material: GlassMaterial {
            frost_radius: element.frost_radius,
            tint_opacity: element.tint_opacity,
            dark_tint: element.dark_tint != 0,
            saturation: 1.0,
            brightness: 0.0,
            contrast: 1.0,
        },
        optics: GlassOptics {
            refraction_strength: element.refraction_strength,
            depth: element.depth,
            dispersion: element.dispersion,
            surface_curvature: 0.6,
        },
        lighting: GlassLighting {
            intensity: element.light_intensity,
            angle_degrees: element.light_angle_degrees,
            splay: element.light_splay,
            shadow_strength: element.shadow_strength,
        },
        // Interaction/style are not yet wired to any shader uniform (see
        // MacroquadGlassRenderer); container/group semantics are OPEN
        // (architecture doc §13/§55.E), so `SGFrame` does not expose them
        // yet either.
        interaction: GlassInteraction::Idle,
        style: GlassStyle::Regular,
    }
}

/// One frame's worth of scene description. `struct_size` must be set to
/// `size_of::<SGFrame>()` by the caller — the ABI-versioning guard from
/// §32: a host built against an older/newer header is rejected with
/// `ErrorInvalidStructSize` instead of silently misreading fields.
#[repr(C)]
pub struct SGFrame {
    pub struct_size: usize,
    pub width: f32,
    pub height: f32,
    pub quality: SGQuality,
    pub reduced_transparency: u8,
    pub reduced_motion: u8,
    pub elements: *const SGGlassElement,
    pub element_count: usize,
}

/// Renders one frame into SparkGlass's internal targets. Call
/// `sg_present` afterwards to composite the result into a bound
/// framebuffer — this split keeps "compute the material" and "where pixels
/// land" independently reusable, matching the still-OPEN render-target
/// question (§24).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sg_render_frame(ctx: *mut SparkGlassContext, frame: *const SGFrame) -> SGResult {
    let Some(ctx) = (unsafe { ctx.as_mut() }) else {
        return SGResult::ErrorNullPointer;
    };
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return SGResult::ErrorNullPointer;
    };
    if frame.struct_size != std::mem::size_of::<SGFrame>() {
        ctx.set_error(format!(
            "sg_render_frame: SGFrame.struct_size mismatch (got {}, expected {}) — rebuild against the current header",
            frame.struct_size,
            std::mem::size_of::<SGFrame>()
        ));
        return SGResult::ErrorInvalidStructSize;
    }
    if frame.element_count > 0 && frame.elements.is_null() {
        ctx.set_error("sg_render_frame: elements is null but element_count > 0");
        return SGResult::ErrorNullPointer;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let elements: &[SGGlassElement] = if frame.element_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(frame.elements, frame.element_count) }
        };
        let mut scene = GlassScene::new(elements.iter().map(element_to_surface).collect());
        scene.quality = frame.quality.into();
        scene.reduced_transparency = frame.reduced_transparency != 0;
        scene.reduced_motion = frame.reduced_motion != 0;
        // `SGFrame` has no container/group fields yet — grouping/container
        // behavior is FUTURE work per architecture doc §13, and the
        // renderer doesn't read `GlassScene::groups` at all today.
        ctx.renderer.resize_if_needed(&ctx.gl, frame.width, frame.height);
        ctx.renderer.render(&ctx.gl, &scene);
    }));
    match result {
        Ok(()) => SGResult::Ok,
        Err(_) => SGResult::ErrorPanic,
    }
}
