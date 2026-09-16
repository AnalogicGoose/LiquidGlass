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
typedef struct LiquidGlassContext LiquidGlassContext;

/*
 * Resolves a GL function name to its address, exactly like
 * eglGetProcAddress / wglGetProcAddress / glXGetProcAddress. The host must
 * have a GL/GLES context current on the calling thread before calling
 * lg_create — SparkGlass never creates its own context.
 */
typedef const void *(*LGGlProc)(const char *name);

typedef int32_t LGResult;
#define LG_OK 0
#define LG_ERROR_NULL_POINTER (-1)
#define LG_ERROR_INVALID_STRUCT_SIZE (-2)
#define LG_ERROR_PANIC (-3)
#define LG_ERROR_INVALID_TEXTURE (-4)

typedef int32_t LGQuality;
#define LG_QUALITY_ULTRA 0
#define LG_QUALITY_HIGH 1
#define LG_QUALITY_MEDIUM 2
#define LG_QUALITY_LOW 3
#define LG_QUALITY_FALLBACK 4

/* One glass surface. Field order must match LGGlassElement in
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
} LGGlassElement;

/* One frame's worth of scene description. struct_size MUST be set to
 * sizeof(LGFrame) — the ABI-versioning guard; a mismatch is rejected with
 * LG_ERROR_INVALID_STRUCT_SIZE instead of silently misread. */
typedef struct {
    size_t struct_size;
    float width;
    float height;
    LGQuality quality;
    uint8_t reduced_transparency;
    uint8_t reduced_motion;
    const LGGlassElement *elements;
    size_t element_count;
} LGFrame;

/*
 * Opaque handle to a texture imported via lg_import_gl_texture. 0 is always
 * invalid (a failed import returns 0), matching the usual C null-handle
 * convention.
 */
typedef uint64_t LGTextureHandle;

LiquidGlassContext *lg_create(LGGlProc loader, float width, float height);
void lg_destroy(LiquidGlassContext *ctx);
LGResult lg_resize(LiquidGlassContext *ctx, float width, float height);

/* Imports a host-owned GL texture, returning an opaque handle the scene can
 * reference via lg_set_backdrop. The host retains ownership — SparkGlass
 * samples it but never destroys it. Returns 0 on failure. */
LGTextureHandle lg_import_gl_texture(LiquidGlassContext *ctx, uint32_t gl_texture_id, float width, float height);
/* Forgets a handle. Does not destroy the underlying GL texture. */
void lg_release_texture(LiquidGlassContext *ctx, LGTextureHandle handle);

/* Draws a previously imported texture as the scene backdrop for the next
 * lg_render_frame call, cover-fit to the frame. */
LGResult lg_set_backdrop(LiquidGlassContext *ctx, LGTextureHandle texture);

LGResult lg_render_frame(LiquidGlassContext *ctx, const LGFrame *frame);
LGResult lg_present(LiquidGlassContext *ctx, int32_t dst_width, int32_t dst_height);
/* Valid until the next call on this context; copy it out if you need to keep it. */
const char *lg_last_error(LiquidGlassContext *ctx);

#ifdef __cplusplus
}
#endif

#endif /* SPARK_GLASS_H */
