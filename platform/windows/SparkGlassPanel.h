// UNVERIFIED SCAFFOLD — see README.md in this directory before trusting
// anything in this file. Nobody has built this against a real Windows App
// SDK / ANGLE package yet; it is not wired into any build.
//
// Sketches the WinUI 3 integration boundary described in the architecture
// doc's target architecture (docs/SparkGlass_ROADMAP.md §3): the host
// (this class) owns the SwapChainPanel, the ANGLE EGL context, and the
// render loop. SparkGlass only ever receives a GL context that's already
// current and a single frame's scene description — it never creates its
// own window, context, or swap chain, matching the same boundary already
// proven on Linux (examples/sandbox.rs, examples/gtk_glarea.rs).

#pragma once

#include <EGL/egl.h>
#include <GLES2/gl2.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>

#include "spark_glass.h"

namespace SparkGlass {

// One SwapChainPanel's worth of SparkGlass state: the ANGLE EGL surface
// backing that panel, plus the SparkGlass context rendering into it.
// Mirrors GlState in examples/gtk_glarea.rs — same "lazily created on
// first render, torn down explicitly" lifecycle, since XAML doesn't
// guarantee a context is current before the panel is actually realized.
class SparkGlassPanel {
public:
    // `panel` must be a live SwapChainPanel; this does not take ownership
    // of it. Does NOT create the EGL display/surface yet — call
    // `EnsureContext` once the panel has a real size (its `SizeChanged`
    // handler is the usual place), same reasoning as GTK's `realize`
    // signal in the Linux integration.
    explicit SparkGlassPanel(winrt::Microsoft::UI::Xaml::Controls::SwapChainPanel const& panel);
    ~SparkGlassPanel();

    SparkGlassPanel(SparkGlassPanel const&) = delete;
    SparkGlassPanel& operator=(SparkGlassPanel const&) = delete;

    // Creates the EGL display/context/surface bound to the panel (if not
    // already created) and the SparkGlassContext bound to it. Safe to call
    // repeatedly; only does real work once per panel lifetime unless
    // `ResetContext` was called (e.g. after a device-lost event, which
    // ANGLE/D3D11 can surface — UNVERIFIED how that manifests here).
    void EnsureContext(uint32_t widthPx, uint32_t heightPx);

    // Renders exactly one frame: makes this panel's EGL context current,
    // fills an SGFrame from `scene` (caller-owned; this class has no
    // opinion on where the scene description comes from), calls
    // sg_render_frame + sg_present, then eglSwapBuffers. Mirrors the
    // render-then-present split already used by every other backend (see
    // architecture doc §24's still-OPEN render-target question — the
    // "host binds its own framebuffer before calling sg_present" contract
    // applies here exactly as it does on GTK/sandbox).
    void RenderFrame(SGFrame const& frame);

    // Tears down the EGL surface/context and the SparkGlassContext. Call
    // this before the panel itself is destroyed.
    void ResetContext();

private:
    winrt::Microsoft::UI::Xaml::Controls::SwapChainPanel m_panel{ nullptr };

    EGLDisplay m_display = EGL_NO_DISPLAY;
    EGLContext m_context = EGL_NO_CONTEXT;
    EGLSurface m_surface = EGL_NO_SURFACE;
    EGLConfig m_config = nullptr;

    SparkGlassContext* m_sparkGlass = nullptr;
};

} // namespace SparkGlass
