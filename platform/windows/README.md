# SparkGlass on Windows — WinUI 3 + ANGLE host scaffold

**Status: unverified scaffold, not a working integration, and Phase 6 is
currently paused.** The team made an explicit call to focus on Linux only
for now — the GitHub Actions Windows CI that used to back this section
(`.github/workflows/windows.yml`) has been removed. Nobody has built or
run this scaffold against real Windows either. Per `AGENTS.md`'s "verify,
don't assume" rule, don't upgrade this file's status to "done" — or reuse
the code below as if it were proven — without actually building it on
Windows first and updating `docs/SparkGlass_IMPLEMENTATION_STATUS.md`.

## What was verified about Windows before Phase 6 was paused

See `docs/SparkGlass_IMPLEMENTATION_STATUS.md`'s Phase 6 section for the
full detail and current status:

- The Rust `cdylib` cross-compiles clean to `x86_64-pc-windows-gnu`
  (mingw) and produces a real PE32+ DLL exporting all 9 `sg_*` C ABI
  functions (`x86_64-w64-mingw32-objdump -p`, checked locally). This one
  doesn't need CI or a Windows machine and can be re-checked any time.
- While the now-removed CI existed, `native-build-and-test` (native MSVC
  build + unit tests + C ABI export check) passed cleanly on a real
  `windows-latest` runner. `c-abi-smoke` (linking `c_smoke.c` against
  Google ANGLE via vcpkg — the first time SparkGlass-rendered pixels ever
  went through ANGLE on any platform) never got past a
  `STATUS_DLL_NOT_FOUND` crash before the workflow was scrapped — see
  `docs/SparkGlass_HANDOFF.md` for the unresolved diagnostic thread if
  Phase 6 picks back up.

What the passing parts proved: the C ABI and Rust dependency graph are
sound on Windows. What was never reached: proof that the GL rendering
pipeline runs correctly through ANGLE on Windows, let alone that a real
WinUI 3 app can host it inside a `SwapChainPanel`, survive XAML's
threading model, or look right composited with native XAML controls. That
needs an actual Windows dev machine with Visual Studio and the Windows App
SDK — none of which exist in this project's environment, which is
ultimately why the team decided to pause here rather than keep debugging
CI blind.

## The intended architecture

Same boundary as every other platform SparkGlass already supports (Linux
sandbox, GTK4): **the host owns the window, the GL/EGL context, and the
event loop; SparkGlass only ever renders into a context that's already
current.** For WinUI 3 specifically, following the pattern Google's own
ANGLE-on-UWP/WinUI samples use:

```text
WinUI 3 App (XAML)
       │
  SwapChainPanel  ──native interop──▶  ISwapChainPanelNative
       │
  ANGLE (libEGL.dll / libGLESv2.dll)
       │  eglGetPlatformDisplayEXT(EGL_PLATFORM_ANGLE_ANGLE, ..., D3D11)
       │  eglCreateWindowSurface(display, config, swapChainPanel, ...)
       │  eglMakeCurrent
       ▼
  Host-owned GL/GLES context (current on the render thread)
       │
       ▼
  SparkGlass C ABI (sg_create / sg_render_frame / sg_present)
       │
       ▼
  eglSwapBuffers
```

`SparkGlassPanel.h`/`.cpp` in this directory sketch that shape in C++.
They are **not wired into any build** — there's no `.vcxproj`/CMake target
for them, deliberately, since nothing here has compiled against a real
Windows App SDK / ANGLE NuGet package yet. Treat them as a starting point
for whoever picks up Phase 6 on an actual Windows machine, not as code to
trust.

## What's genuinely uncertain and needs verifying on real Windows

- The exact `IPropertySet` keys ANGLE expects for
  `eglCreateWindowSurface`'s `SwapChainPanel` native-window type
  (`EGLNativeWindowTypeProperty`, render-surface-size / native-resolution
  properties) are an ANGLE-internal, version-sensitive contract — the
  names used below match Google's published UWP/WinUI ANGLE samples at
  the time this was written, but must be checked against whatever ANGLE
  version actually ships (NuGet `ANGLE.WindowsStore` vs. a vendored build).
- Which thread XAML requires the render loop to run on, and how that
  interacts with `SwapChainPanel` resize events, is not something that
  can be reasoned about correctly without running it.
- Packaging: a WinUI 3 app needs MSIX packaging and the Windows App SDK
  runtime; none of that has been set up or tested here.

## What to do next on a real Windows machine

1. Create a WinUI 3 (Windows App SDK) C++ project in Visual Studio.
2. Add the ANGLE NuGet package (`ANGLE.WindowsStore`) or vendor prebuilt
   ANGLE binaries.
3. Reference `include/spark_glass.h` and link against the `spark_glass.dll`
   built by `cargo build --release --lib` (native MSVC target — don't reuse
   the mingw cross-build for a real app; it's for CI verification only).
4. Wire up `SparkGlassPanel` (or replace it with whatever the real
   integration ends up looking like) against an actual `SwapChainPanel` in
   a XAML page, get a single frame rendering, and screenshot it.
5. Update `docs/SparkGlass_IMPLEMENTATION_STATUS.md`'s Phase 6 section
   with what was actually verified — don't mark it "done" until there's a
   real screenshot, same rule as every other platform here.
