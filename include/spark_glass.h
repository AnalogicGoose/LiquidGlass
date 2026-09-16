/*
 * SparkGlass C ABI v0 — hand-written to mirror src/ffi/ (context.rs,
 * frame.rs, error.rs) field-for-field and function-for-function.
 *
 * Result/quality codes are declared as plain int32_t here rather than a C
 * `enum`, deliberately: a C compiler's `enum` underlying type is
 * implementation-defined, while the Rust side is `#[repr(i32)]`, which is a
 * fixed 4-byte signed integer. int32_t is the type that is actually
 * guaranteed to match on both sides.
 *
 * See docs/SparkGlass_MASTER_ARCHITECTURE.md §30-§34 for the ABI rules this
 * follows (opaque handle, no Rust layout assumptions, struct_size
 * versioning, stable numeric error codes).
 */
#ifndef SPARK_GLASS_H
#define SPARK_GLASS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque — never inspect its layout, only ever hold a pointer to it. */
typedef struct SparkGlassContext SparkGlassContext;

/*
 * Resolves a GL function name to its address, exactly like
 * eglGetProcAddress / wglGetProcAddress / glXGetProcAddress. The host must
 * have a GL/GLES context current on the calling thread before calling
 * sg_create — SparkGlass never creates its own context.
 */
typedef const void *(*SGGlProc)(const char *name);

typedef int32_t SGResult;
#define SG_OK 0
#define SG_ERROR_NULL_POINTER (-1)
#define SG_ERROR_INVALID_STRUCT_SIZE (-2)
#define SG_ERROR_PANIC (-3)
#define SG_ERROR_INVALID_TEXTURE (-4)

typedef int32_t SGQuality;
#define SG_QUALITY_ULTRA 0
#define SG_QUALITY_HIGH 1
#define SG_QUALITY_MEDIUM 2
#define SG_QUALITY_LOW 3
#define SG_QUALITY_FALLBACK 4

/* One glass surface. Field order must match SGGlassElement in
 * src/ffi/frame.rs exactly. */
typedef struct {
    uint64_t id;
    float center_x;
    float center_y;
    float size_x;
    float size_y;
    float radius;
    float smoothing;
    float refraction_strength;
    float depth;
    float dispersion;
    float frost_radius;
    float light_intensity;
    float light_angle_degrees;
    float light_splay;
    float tint_opacity;
    uint8_t dark_tint; /* 0 = light tint, non-zero = dark tint */
    float shadow_strength;
} SGGlassElement;

/* One frame's worth of scene description. struct_size MUST be set to
 * sizeof(SGFrame) — the ABI-versioning guard; a mismatch is rejected with
 * SG_ERROR_INVALID_STRUCT_SIZE instead of silently misread. */
typedef struct {
    size_t struct_size;
    float width;
    float height;
    SGQuality quality;
    uint8_t reduced_transparency;
    uint8_t reduced_motion;
    const SGGlassElement *elements;
    size_t element_count;
} SGFrame;

/*
 * Opaque handle to a texture imported via sg_import_gl_texture. 0 is always
 * invalid (a failed import returns 0), matching the usual C null-handle
 * convention.
 */
typedef uint64_t SGTextureHandle;

SparkGlassContext *sg_create(SGGlProc loader, float width, float height);
void sg_destroy(SparkGlassContext *ctx);
SGResult sg_resize(SparkGlassContext *ctx, float width, float height);

/* Imports a host-owned GL texture, returning an opaque handle the scene can
 * reference via sg_set_backdrop. The host retains ownership — SparkGlass
 * samples it but never destroys it. Returns 0 on failure. */
SGTextureHandle sg_import_gl_texture(SparkGlassContext *ctx, uint32_t gl_texture_id, float width, float height);
/* Forgets a handle. Does not destroy the underlying GL texture. */
void sg_release_texture(SparkGlassContext *ctx, SGTextureHandle handle);

/* Draws a previously imported texture as the scene backdrop for the next
 * sg_render_frame call, cover-fit to the frame. */
SGResult sg_set_backdrop(SparkGlassContext *ctx, SGTextureHandle texture);

SGResult sg_render_frame(SparkGlassContext *ctx, const SGFrame *frame);
SGResult sg_present(SparkGlassContext *ctx, int32_t dst_width, int32_t dst_height);
/* Valid until the next call on this context; copy it out if you need to keep it. */
const char *sg_last_error(SparkGlassContext *ctx);

#ifdef __cplusplus
}
#endif

#endif /* SPARK_GLASS_H */
