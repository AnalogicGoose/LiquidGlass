/*
 * Phase 2 exit-condition proof, taken literally: a plain C program, built
 * with a plain C compiler, linking directly against libspark_glass.so
 * and spark_glass.h — no Rust anywhere in this file or its build.
 *
 * Uses a headless EGL pbuffer context (no window/display server dependency
 * beyond an EGL-capable driver) so this can run in any environment that can
 * build the library itself.
 *
 * Build & run (from the repository root):
 *   cargo build --lib
 *   gcc c_smoke/main.c -Iinclude -lEGL -lGLESv2 \
 *       -L$(cargo metadata --no-deps --format-version 1 | python3 -c \
 *           'import json,sys;print(json.load(sys.stdin)["target_directory"])')/debug \
 *       -lspark_glass -Wl,-rpath,'$ORIGIN' -o /tmp/spark_glass_c_smoke
 *   LD_LIBRARY_PATH=<target-dir>/debug /tmp/spark_glass_c_smoke
 */
#include <EGL/egl.h>
#include <GLES2/gl2.h>
#include <stdio.h>
#include <stdlib.h>

#include "spark_glass.h"

static const void *gl_proc(const char *name) {
    return (const void *) eglGetProcAddress(name);
}

int main(void) {
    EGLDisplay display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    if (display == EGL_NO_DISPLAY) {
        fprintf(stderr, "eglGetDisplay failed\n");
        return 1;
    }
    EGLint major, minor;
    if (!eglInitialize(display, &major, &minor)) {
        fprintf(stderr, "eglInitialize failed\n");
        return 1;
    }
    printf("EGL %d.%d\n", major, minor);

    EGLint config_attribs[] = {
        EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
        EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
        EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
        EGL_NONE,
    };
    EGLConfig config;
    EGLint num_configs = 0;
    if (!eglChooseConfig(display, config_attribs, &config, 1, &num_configs) || num_configs < 1) {
        fprintf(stderr, "eglChooseConfig failed\n");
        return 1;
    }

    EGLint pbuffer_attribs[] = {EGL_WIDTH, 800, EGL_HEIGHT, 600, EGL_NONE};
    EGLSurface surface = eglCreatePbufferSurface(display, config, pbuffer_attribs);
    if (surface == EGL_NO_SURFACE) {
        fprintf(stderr, "eglCreatePbufferSurface failed\n");
        return 1;
    }

    eglBindAPI(EGL_OPENGL_ES_API);
    EGLint context_attribs[] = {EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE};
    EGLContext context = eglCreateContext(display, config, EGL_NO_CONTEXT, context_attribs);
    if (context == EGL_NO_CONTEXT) {
        fprintf(stderr, "eglCreateContext failed\n");
        return 1;
    }

    if (!eglMakeCurrent(display, surface, surface, context)) {
        fprintf(stderr, "eglMakeCurrent failed\n");
        return 1;
    }

    SparkGlassContext *ctx = sg_create(gl_proc, 800.0f, 600.0f);
    if (!ctx) {
        fprintf(stderr, "sg_create returned NULL\n");
        return 1;
    }
    printf("sg_create: OK\n");

    /* A trivial 2x2 texture, uploaded directly with GLES2 calls (this
     * program links against libGLESv2 itself) — stands in for whatever
     * texture a real host would already have from its own rendering. */
    GLuint gl_texture = 0;
    glGenTextures(1, &gl_texture);
    glBindTexture(GL_TEXTURE_2D, gl_texture);
    unsigned char pixels[2 * 2 * 4] = {
        255, 0, 0, 255, 0, 255, 0, 255,
        0, 0, 255, 255, 255, 255, 0, 255,
    };
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, 2, 2, 0, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
    glBindTexture(GL_TEXTURE_2D, 0);

    SGTextureHandle backdrop = sg_import_gl_texture(ctx, gl_texture, 2.0f, 2.0f);
    if (backdrop == 0) {
        fprintf(stderr, "sg_import_gl_texture failed: %s\n", sg_last_error(ctx));
        return 1;
    }
    printf("sg_import_gl_texture: OK (handle=%llu)\n", (unsigned long long) backdrop);

    SGResult backdrop_result = sg_set_backdrop(ctx, backdrop);
    if (backdrop_result != SG_OK) {
        fprintf(stderr, "sg_set_backdrop failed: %d (%s)\n", backdrop_result, sg_last_error(ctx));
        return 1;
    }
    printf("sg_set_backdrop: OK\n");

    /* An unknown handle must be rejected, not silently accepted. */
    SGResult bad_backdrop_result = sg_set_backdrop(ctx, backdrop + 1000);
    if (bad_backdrop_result != SG_ERROR_INVALID_TEXTURE) {
        fprintf(stderr, "expected SG_ERROR_INVALID_TEXTURE, got %d\n", bad_backdrop_result);
        return 1;
    }
    printf("unknown-handle guard: OK (%s)\n", sg_last_error(ctx));

    sg_release_texture(ctx, backdrop);
    printf("sg_release_texture: OK\n");

    SGGlassElement element = {0};
    element.id = 1;
    element.center_x = 400.0f;
    element.center_y = 300.0f;
    element.size_x = 300.0f;
    element.size_y = 200.0f;
    element.radius = 24.0f;
    element.smoothing = 0.6f;
    element.refraction_strength = 2.0f;
    element.depth = 30.0f;
    element.dispersion = 0.2f;
    element.frost_radius = 6.0f;
    element.light_intensity = 0.25f;
    element.light_angle_degrees = 0.0f;
    element.light_splay = 0.2f;
    element.tint_opacity = 0.15f;
    element.dark_tint = 0;
    element.shadow_strength = 1.0f;

    SGFrame frame = {0};
    frame.struct_size = sizeof(SGFrame);
    frame.width = 800.0f;
    frame.height = 600.0f;
    frame.quality = SG_QUALITY_HIGH;
    frame.reduced_transparency = 0;
    frame.reduced_motion = 0;
    frame.elements = &element;
    frame.element_count = 1;

    SGResult render_result = sg_render_frame(ctx, &frame);
    if (render_result != SG_OK) {
        fprintf(stderr, "sg_render_frame failed: %d (%s)\n", render_result, sg_last_error(ctx));
        return 1;
    }
    printf("sg_render_frame: OK\n");

    SGResult present_result = sg_present(ctx, 800, 600);
    if (present_result != SG_OK) {
        fprintf(stderr, "sg_present failed: %d\n", present_result);
        return 1;
    }
    printf("sg_present: OK\n");

    /* sg_resize was never exercised by this smoke test before — resize to
     * a smaller size (staying within the 800x600 pbuffer so sg_present's
     * viewport call has real framebuffer to draw into) and confirm a full
     * render/present cycle still succeeds at the new size. */
    SGResult resize_result = sg_resize(ctx, 400.0f, 300.0f);
    if (resize_result != SG_OK) {
        fprintf(stderr, "sg_resize failed: %d (%s)\n", resize_result, sg_last_error(ctx));
        return 1;
    }
    SGFrame resized_frame = frame;
    resized_frame.width = 400.0f;
    resized_frame.height = 300.0f;
    element.center_x = 200.0f;
    element.center_y = 150.0f;
    element.size_x = 150.0f;
    element.size_y = 100.0f;
    SGResult resized_render_result = sg_render_frame(ctx, &resized_frame);
    if (resized_render_result != SG_OK) {
        fprintf(stderr, "sg_render_frame (post-resize) failed: %d (%s)\n", resized_render_result, sg_last_error(ctx));
        return 1;
    }
    SGResult resized_present_result = sg_present(ctx, 400, 300);
    if (resized_present_result != SG_OK) {
        fprintf(stderr, "sg_present (post-resize) failed: %d\n", resized_present_result);
        return 1;
    }
    printf("sg_resize + re-render + re-present: OK\n");

    /* Deliberately exercise the struct-versioning guard: an SGFrame from a
     * mismatched header must be rejected, not silently misread. */
    SGFrame bad_frame = frame;
    bad_frame.struct_size = sizeof(SGFrame) + 8;
    SGResult bad_result = sg_render_frame(ctx, &bad_frame);
    if (bad_result != SG_ERROR_INVALID_STRUCT_SIZE) {
        fprintf(stderr, "expected SG_ERROR_INVALID_STRUCT_SIZE, got %d\n", bad_result);
        return 1;
    }
    printf("struct_size guard: OK (%s)\n", sg_last_error(ctx));

    /* SG_ERROR_NULL_POINTER is declared but was never actually exercised
     * anywhere before this: a C caller passing NULL must get a clean error
     * code back, not a segfault. If any of these crash, this whole program
     * crashes instead of printing "All checks passed" — that failure mode
     * *is* the test. */
    if (sg_resize(NULL, 100.0f, 100.0f) != SG_ERROR_NULL_POINTER) {
        fprintf(stderr, "sg_resize(NULL, ...) did not return SG_ERROR_NULL_POINTER\n");
        return 1;
    }
    if (sg_import_gl_texture(NULL, gl_texture, 2.0f, 2.0f) != 0) {
        fprintf(stderr, "sg_import_gl_texture(NULL, ...) did not return 0\n");
        return 1;
    }
    sg_release_texture(NULL, 1); /* must be a safe no-op */
    if (sg_set_backdrop(NULL, 1) != SG_ERROR_NULL_POINTER) {
        fprintf(stderr, "sg_set_backdrop(NULL, ...) did not return SG_ERROR_NULL_POINTER\n");
        return 1;
    }
    if (sg_render_frame(NULL, &frame) != SG_ERROR_NULL_POINTER) {
        fprintf(stderr, "sg_render_frame(NULL, ...) did not return SG_ERROR_NULL_POINTER\n");
        return 1;
    }
    if (sg_render_frame(ctx, NULL) != SG_ERROR_NULL_POINTER) {
        fprintf(stderr, "sg_render_frame(ctx, NULL) did not return SG_ERROR_NULL_POINTER\n");
        return 1;
    }
    if (sg_present(NULL, 100, 100) != SG_ERROR_NULL_POINTER) {
        fprintf(stderr, "sg_present(NULL, ...) did not return SG_ERROR_NULL_POINTER\n");
        return 1;
    }
    if (sg_last_error(NULL) != NULL) {
        fprintf(stderr, "sg_last_error(NULL) did not return NULL\n");
        return 1;
    }
    sg_destroy(NULL); /* must be a safe no-op */
    printf("NULL-pointer safety: OK (every context-taking function survived a NULL ctx)\n");

    sg_destroy(ctx);
    printf("sg_destroy: OK\nAll checks passed.\n");
    return 0;
}
