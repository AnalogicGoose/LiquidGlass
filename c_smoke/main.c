/*
 * Phase 2 exit-condition proof, taken literally: a plain C program, built
 * with a plain C compiler, linking directly against libspark_glass_poc.so
 * and spark_glass.h — no Rust anywhere in this file or its build.
 *
 * Uses a headless EGL pbuffer context (no window/display server dependency
 * beyond an EGL-capable driver) so this can run in any environment that can
 * build the library itself.
 *
 * Build & run (from the repository root):
 *   cargo build --lib
 *   gcc c_smoke/main.c -Iinclude -lEGL -lGLESv2 \
 *       -L$(cargo metadata --no-deps --format-version1 | python3 -c \
 *           'import json,sys;print(json.load(sys.stdin)["target_directory"])')/debug \
 *       -lspark_glass_poc -Wl,-rpath,'$ORIGIN' -o /tmp/spark_glass_c_smoke
 *   LD_LIBRARY_PATH=<target-dir>/debug /tmp/spark_glass_c_smoke
 */
#include <EGL/egl.h>
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

    LiquidGlassContext *ctx = lg_create(gl_proc, 800.0f, 600.0f);
    if (!ctx) {
        fprintf(stderr, "lg_create returned NULL\n");
        return 1;
    }
    printf("lg_create: OK\n");

    LGGlassElement element = {0};
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

    LGFrame frame = {0};
    frame.struct_size = sizeof(LGFrame);
    frame.width = 800.0f;
    frame.height = 600.0f;
    frame.quality = LG_QUALITY_HIGH;
    frame.reduced_transparency = 0;
    frame.reduced_motion = 0;
    frame.elements = &element;
    frame.element_count = 1;

    LGResult render_result = lg_render_frame(ctx, &frame);
    if (render_result != LG_OK) {
        fprintf(stderr, "lg_render_frame failed: %d (%s)\n", render_result, lg_last_error(ctx));
        return 1;
    }
    printf("lg_render_frame: OK\n");

    LGResult present_result = lg_present(ctx, 800, 600);
    if (present_result != LG_OK) {
        fprintf(stderr, "lg_present failed: %d\n", present_result);
        return 1;
    }
    printf("lg_present: OK\n");

    /* Deliberately exercise the struct-versioning guard: an LGFrame from a
     * mismatched header must be rejected, not silently misread. */
    LGFrame bad_frame = frame;
    bad_frame.struct_size = sizeof(LGFrame) + 8;
    LGResult bad_result = lg_render_frame(ctx, &bad_frame);
    if (bad_result != LG_ERROR_INVALID_STRUCT_SIZE) {
        fprintf(stderr, "expected LG_ERROR_INVALID_STRUCT_SIZE, got %d\n", bad_result);
        return 1;
    }
    printf("struct_size guard: OK (%s)\n", lg_last_error(ctx));

    lg_destroy(ctx);
    printf("lg_destroy: OK\nAll checks passed.\n");
    return 0;
}
