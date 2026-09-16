// UNVERIFIED SCAFFOLD — see README.md and SparkGlassPanel.h in this
// directory. Not built, not run, not wired into any project. The EGL
// property-set keys below (EGLNativeWindowTypeProperty and friends) are an
// ANGLE-internal contract for the SwapChainPanel native-window type;
// they're written here matching Google's published UWP/WinUI ANGLE
// samples, but MUST be checked against whatever ANGLE version actually
// ships before trusting this compiles, let alone runs.

#include "SparkGlassPanel.h"

#include <windows.foundation.collections.h>
#include <windows.ui.xaml.media.dxinterop.h> // ISwapChainPanelNative
#include <wrl/client.h>

using namespace winrt;
using namespace Microsoft::UI::Xaml::Controls;
using Microsoft::WRL::ComPtr;

namespace SparkGlass {

namespace {

// Resolves a GL function pointer the same way every other SparkGlass host
// does (see examples/gtk_glarea.rs's GlLoader, examples/sandbox.rs) — the
// C ABI only ever asks for `eglGetProcAddress`-shaped resolution
// (spark_glass.h's SGGlProc), never anything platform-specific.
const void* ResolveGlProc(char const* name) {
    return reinterpret_cast<const void*>(eglGetProcAddress(name));
}

} // namespace

SparkGlassPanel::SparkGlassPanel(SwapChainPanel const& panel) : m_panel(panel) {}

SparkGlassPanel::~SparkGlassPanel() {
    ResetContext();
}

void SparkGlassPanel::EnsureContext(uint32_t widthPx, uint32_t heightPx) {
    if (m_sparkGlass != nullptr) {
        return; // Already created — UNVERIFIED: does this need to handle
                // resize by recreating the EGL surface, or can ANGLE's
                // surface be resized in place? Needs checking against
                // real SwapChainPanel resize behavior.
    }

    // EGL_PLATFORM_ANGLE_* constants come from ANGLE's egl/eglext_angle.h,
    // not core EGL — that header ships with the ANGLE package, not with
    // this repo.
    EGLint displayAttribs[] = {
        EGL_PLATFORM_ANGLE_TYPE_ANGLE, EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE,
        EGL_NONE,
    };
    m_display = eglGetPlatformDisplayEXT(EGL_PLATFORM_ANGLE_ANGLE, EGL_DEFAULT_DISPLAY, displayAttribs);
    if (m_display == EGL_NO_DISPLAY) {
        return; // TODO: surface this as a real error once there's a host
                // error-reporting story on this platform.
    }

    EGLint major = 0, minor = 0;
    if (!eglInitialize(m_display, &major, &minor)) {
        return;
    }

    EGLint configAttribs[] = {
        EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
        EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
        EGL_NONE,
    };
    EGLint numConfigs = 0;
    if (!eglChooseConfig(m_display, configAttribs, &m_config, 1, &numConfigs) || numConfigs < 1) {
        return;
    }

    // Native-window interop: get ISwapChainPanelNative from the XAML
    // panel, same COM interop every ANGLE-on-XAML sample uses. This part
    // is well-established, unlike the property-set keys below.
    ComPtr<ISwapChainPanelNative> panelNative;
    winrt::com_ptr<::IUnknown> panelUnknown = m_panel.as<::IUnknown>();
    if (FAILED(panelUnknown->QueryInterface(IID_PPV_ARGS(&panelNative)))) {
        return;
    }

    // UNVERIFIED: the specific property keys ANGLE's UWP/WinUI backend
    // reads off an IPropertySet passed as the EGLNativeWindowType. This
    // shape matches Google's public ANGLE-on-UWP samples' use of
    // EGLNativeWindowTypeProperty / EGLRenderSurfaceSizeProperty, but
    // whether WinUI 3's SwapChainPanel (as opposed to UWP's) takes the
    // same contract has not been checked against real ANGLE source.
    EGLint surfaceAttribs[] = {
        EGL_WIDTH, static_cast<EGLint>(widthPx),
        EGL_HEIGHT, static_cast<EGLint>(heightPx),
        EGL_NONE,
    };
    m_surface = eglCreateWindowSurface(
        m_display, m_config,
        reinterpret_cast<EGLNativeWindowType>(panelNative.Get()),
        surfaceAttribs);
    if (m_surface == EGL_NO_SURFACE) {
        return;
    }

    EGLint contextAttribs[] = { EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE };
    m_context = eglCreateContext(m_display, m_config, EGL_NO_CONTEXT, contextAttribs);
    if (m_context == EGL_NO_CONTEXT) {
        return;
    }

    if (!eglMakeCurrent(m_display, m_surface, m_surface, m_context)) {
        return;
    }

    // Exactly the same call every other backend makes — see
    // examples/sandbox.rs, examples/gtk_glarea.rs, c_smoke/main.c. The C
    // ABI has no idea it's on Windows; it only ever sees "a GL context is
    // current, here's a proc-address resolver."
    m_sparkGlass = sg_create(ResolveGlProc, static_cast<float>(widthPx), static_cast<float>(heightPx));
}

void SparkGlassPanel::RenderFrame(SGFrame const& frame) {
    if (m_sparkGlass == nullptr) {
        return;
    }
    eglMakeCurrent(m_display, m_surface, m_surface, m_context);

    sg_render_frame(m_sparkGlass, &frame);
    // UNVERIFIED: which framebuffer is bound here (the default one ANGLE
    // set up for `m_surface`?) and whether sg_present's "composite into
    // whatever's currently bound" contract (architecture doc §24, the
    // render-target lesson in docs/SparkGlass_HANDOFF.md) holds unchanged
    // on ANGLE/D3D11 the same way it does on desktop GL/EGL. This is
    // exactly the class of bug that bit the Linux renderer once already —
    // don't assume it's fine here without checking.
    sg_present(m_sparkGlass, static_cast<int32_t>(frame.width), static_cast<int32_t>(frame.height));

    eglSwapBuffers(m_display, m_surface);
}

void SparkGlassPanel::ResetContext() {
    if (m_sparkGlass != nullptr) {
        sg_destroy(m_sparkGlass);
        m_sparkGlass = nullptr;
    }
    if (m_display != EGL_NO_DISPLAY) {
        eglMakeCurrent(m_display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
        if (m_context != EGL_NO_CONTEXT) {
            eglDestroyContext(m_display, m_context);
            m_context = EGL_NO_CONTEXT;
        }
        if (m_surface != EGL_NO_SURFACE) {
            eglDestroySurface(m_display, m_surface);
            m_surface = EGL_NO_SURFACE;
        }
        eglTerminate(m_display);
        m_display = EGL_NO_DISPLAY;
    }
}

} // namespace SparkGlass
