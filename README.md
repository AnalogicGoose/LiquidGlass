# SparkGlass

A cross-platform GPU material engine, in Rust, that reproduces Apple's **Liquid Glass** — refraction, frost, edge lighting, backdrop-aware blur, shadow — for native desktop apps on Windows and Linux.

> Visual fidelity is the product. Everything else is infrastructure.

SparkGlass owns the glass material and nothing else. The host owns the window, the UI toolkit, the event loop, and the GL context; SparkGlass never creates any of them. A host describes a scene, SparkGlass renders it into whatever GL context and framebuffer the host already has current, through a stable C ABI.

## Status

The renderer, the `glow` GL backend, the C ABI, and Linux (GTK4) integration are implemented and independently verified — not just "compiles":

- **Four independent host integrations render byte-identical output**: a standalone `winit`+`glutin` sandbox, the `extern "C"` ABI called from Rust, a real GTK4 `GtkGLArea` widget, and a plain C program (`gcc`, no Rust) linked against the compiled `.so`.
- **Native GTK widgets composite correctly** over the live glass render with zero GL state save/restore on SparkGlass's side — real evidence, not a guess, toward one of the project's open architectural questions.
- A real render-target bug was found and fixed along the way: on-screen output was silently broken in every example despite passing same-process pixel-readback checks. Worth reading if you're about to trust a `glReadPixels` capture as proof a frame reached the screen.
- The Macroquad proof of concept (`cargo run`) remains the visual reference every new backend is checked against, pixel by pixel.

Full details, what's still missing, and how each claim above was verified: **[`docs/SparkGlass_IMPLEMENTATION_STATUS.md`](docs/SparkGlass_IMPLEMENTATION_STATUS.md)**.

The architectural rationale — why the host owns the context, why SDF, why GL before Vulkan, the full phase plan, and what AI agents or contributors should never do to this codebase: **[`docs/SparkGlass_MASTER_ARCHITECTURE.md`](docs/SparkGlass_MASTER_ARCHITECTURE.md)**.

## Quick start

```bash
cargo run                          # Macroquad reference — the visual baseline
cargo run --example sandbox        # windowless-core renderer: winit + glutin, no Macroquad
cargo run --example ffi_smoke      # the same renderer, driven only through the extern "C" ABI
cargo run --example gtk_glarea     # embedded in a real GTK4 GtkGLArea
```

Building the pure-C proof (`c_smoke/`) needs `gcc` plus EGL/GLES2 headers — see the comment at the top of `c_smoke/main.c` for exact build/run commands.

## License

MIT — see [`LICENSE`](LICENSE).
