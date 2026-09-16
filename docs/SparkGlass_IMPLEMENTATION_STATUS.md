# SparkGlass — Implementation Status

> Companion to [`SparkGlass_MASTER_ARCHITECTURE.md`](SparkGlass_MASTER_ARCHITECTURE.md), which is the design rationale and changes rarely. This document tracks what has actually been built and verified against that design, and is expected to change often. Phase numbers match §54 of the master doc.

## Summary

| Phase | What | Status |
|---|---|---|
| 0 | Preserve the Macroquad reference | Done — `cargo run` is still the visual baseline |
| 1 | Freeze scene/material semantics | Mostly done informally in `src/glass.rs`; not yet frozen on purpose |
| 2 | C ABI v0 | Done — `src/ffi/`, proven from a real C program, not just Rust |
| 3 | `glow` GL renderer | Done — `src/backend/gl/` |
| 4 | Standalone windowless-core sandbox | Done — `examples/sandbox.rs` |
| 5 | Linux (GTK4) integration | Done — `examples/gtk_glarea.rs`, `examples/gtk_glarea_overlay.rs` |
| 6 | Windows (WinUI 3 + ANGLE) integration | Not started — no Windows environment available to build or verify against |
| 7 | GoosicReborn integration | Not started |
| 8 | Advanced fidelity/performance | Not started |
| 9 | Native Vulkan backend | Not started (not the current target per the master doc) |

Everything below is either implemented-and-verified or an explicit gap — nothing here is aspirational. If it doesn't have a "Status" note, it hasn't been built.

---

## Phase 2 — C ABI v0

`src/ffi/` (`context.rs`, `frame.rs`, `error.rs`, `texture.rs`) exposes:

```text
lg_create / lg_destroy / lg_resize /
lg_import_gl_texture / lg_release_texture / lg_set_backdrop /
lg_render_frame / lg_present / lg_last_error
```

Backdrop textures go through an explicit import step —
`lg_import_gl_texture(ctx, gl_texture_id, width, height) -> LGTextureHandle`
(an opaque `u64`, `0` always invalid) — rather than taking a raw `GLuint`
directly on every call. This follows §22 of the master doc, which explicitly
says the public/FFI surface must not depend directly on `GLuint`:
`GLuint → lg_import_gl_texture(...) → LGTextureHandle → scene references the
handle`. `lg_set_backdrop(ctx, handle)` and `lg_release_texture(ctx, handle)`
consume it from there. The host still owns the underlying GL texture in all
of this — `lg_release_texture` only forgets SparkGlass's reference, it never
calls `glDeleteTextures`.

All `extern "C"`, all panic-safe (`catch_unwind` at every boundary — no Rust
unwind ever crosses it), all operating only on the opaque `LiquidGlassContext`
pointer and POD `LGFrame`/`LGGlassElement` structs.

Verified two ways:

- **`examples/ffi_smoke.rs`** drives the renderer through these functions
  only (no `GlGlassRenderer`, no `GlassScene`) and its captured output is
  byte-identical to `examples/sandbox.rs`'s direct-API output — the wrapper
  adds no behavioral divergence.
- **`c_smoke/main.c` + `include/spark_glass.h`** prove the exit condition
  literally: a plain C program, built with `gcc`, linking directly against
  the crate's `cdylib` output (`libspark_glass_poc.so`) — no Rust anywhere in
  that build. It uploads a real GLES2 texture, imports it with
  `lg_import_gl_texture`, sets it as the backdrop, and confirms an unknown
  handle is rejected with `LG_ERROR_INVALID_TEXTURE`; then it creates a
  headless EGL pbuffer context and calls
  `lg_create`/`lg_render_frame`/`lg_present`/`lg_destroy`, and deliberately
  submits an `LGFrame` with a wrong `struct_size` to confirm the ABI-version
  guard is actually enforced across a real FFI boundary. `sizeof(LGFrame)`
  matched between `gcc` and `rustc` (40 bytes) with no manual padding in the
  header, confirming the `#[repr(C)]` layout really is what the header
  claims. Build/run instructions are in a comment at the top of that file.

Result codes and quality levels are declared in the C header as `int32_t`,
not a C `enum` — a C enum's underlying type is implementation-defined, while
the Rust side is `#[repr(i32)]`, a fixed 4-byte signed integer. `int32_t` is
the type actually guaranteed to match on both sides.

**Not done:** container/interaction fields on `LGGlassElement` (grouping is
still FUTURE work per the master doc §13).

---

## Phase 3 — `glow` GL renderer

`src/backend/gl/` (`renderer.rs`, `shader.rs`, `target.rs`, `texture.rs`)
reproduces `MacroquadGlassRenderer`'s exact multi-pass ping-pong pipeline.
`glass.frag` / `glass_mask.frag` / `blur.frag` are reused verbatim — only the
vertex stage and host-facing plumbing are new, per the rule that shaders are
the material and the vertex stage is just mechanics.

**Not done:** the GL state-preservation contract (see "Findings" below) is
still informally "leave it however the last draw left it," not a documented
contract.

---

## Phase 4 — Standalone sandbox

`examples/sandbox.rs` is a `winit` + `glutin` host with zero Macroquad
dependency. Run it:

```bash
cargo run --example sandbox
```

Visual parity with the Macroquad reference was confirmed by side-by-side
capture at 1280×800 under the "Clear" profile — set
`SPARK_GLASS_SANDBOX_CAPTURE=<path>` to dump a PNG after a few frames, and
compare against `SPARK_GLASS_CAPTURE=<path>` on the reference binary
(`cargo run`). Both are debug/test-tooling readbacks (architecture doc §43),
never used on the normal render path.

---

## Phase 5 — Linux (GTK4) integration

`examples/gtk_glarea.rs` embeds `GlGlassRenderer` inside a real
`GtkGLArea::connect_render` callback:

```bash
cargo run --example gtk_glarea
```

`GLArea::connect_render` hands control to us with GTK's `GdkGLContext`
already current and its own framebuffer already bound; the renderer never
creates a context or window, and (after the fix below) never leaves a
foreign framebuffer bound either — it only uses what's current, exactly the
architecture doc's context-ownership contract.

GL function addresses are resolved via `eglGetProcAddress`, dlopened
directly from `libEGL.so.1`. This is *not* the `epoxy` crate gtk-rs examples
normally use for this — `epoxy`'s `gl_generator` dependency requires a
yanked `xml-rs` release with no fixed version available, making it currently
unusable via a fresh `cargo` resolve.

`examples/gtk_glarea_overlay.rs` goes further: real native GTK widgets
(`Button`, `Switch`, `Label`) in a `gtk::Overlay` on top of the same
`GLArea`, so GTK's own widget rendering runs immediately after ours every
frame — see "GL state isolation" below for what that showed.

**Not done:** the GLX fallback path for X11 sessions (only EGL is wired up;
Wayland is what this was validated on).

---

## Findings

Two things were discovered and resolved while verifying Phase 5, both
directly relevant to open questions in the master doc (§55.B render target,
§55.C GL state).

### The render-target bug (§55.B)

`GlGlassRenderer::render()` ended by binding its own internal `output`
framebuffer and never rebound anything afterward. `present()` — which trusts
"whatever's currently bound is the host's target" (the simplest reading of
render-target option A) — was therefore silently compositing into that
internal target instead of the host's actual framebuffer, on every call, in
every example.

This was **invisible** to the verification method used up to that point: a
same-process `glReadPixels` capture taken immediately after `present()` still
read back pixel-perfect output, because it was reading `output` right back —
not the host's real target. `examples/sandbox.rs`, `examples/ffi_smoke.rs`,
and `examples/gtk_glarea.rs` all "passed" byte-identical capture comparisons
against each other while their actual on-screen windows were blank. It only
surfaced once an *external* screenshot of a real running window was taken.

**Fix:** `draw_backdrop()` and `render()` now snapshot
`GL_DRAW_FRAMEBUFFER_BINDING` on entry and restore it before returning, so
`present()`'s original "draw into whatever's current" logic is actually
correct. Verified with real screen captures (not `glReadPixels`) on
`sandbox` and `gtk_glarea` after the fix, and confirmed the internal-capture
regression tests still match byte-for-byte.

**Lesson for future verification work:** a readback taken from inside the
same render call cannot tell you whether the *host* actually received the
frame — only an external capture (or a human looking at the screen) can. Any
future backend (WinUI/ANGLE included) should be checked the same way before
being marked verified.

### GL state isolation experiment (§55.C)

`examples/gtk_glarea_overlay.rs` puts real native GTK widgets in a
`gtk::Overlay` on top of the `GLArea`. With **no** GL state save/restore on
SparkGlass's side beyond the framebuffer-binding fix above — VAO, active
program, blend state, and texture units are all left however the last draw
call set them — the native widgets rendered perfectly in a live screen
capture: crisp, correctly styled, no artifacts.

This is evidence (not proof) that GTK4's GSK-based renderer does not depend
on inherited GL state and re-establishes whatever it needs before drawing,
at least for this state footprint (no depth/stencil/scissor touched) on this
driver/Mesa stack. It says nothing about WinUI, and nothing about a larger
state footprint than this renderer currently touches. Treat it as one data
point toward "documented minimal state contract," not a closed decision.

---

## Automated regression check

`scripts/verify_backends.sh` automates what had been a manual `cmp`-by-hand
process throughout this work: it builds everything, captures `sandbox`,
`ffi_smoke`, and (if a display is available) `gtk_glarea`, asserts they're
byte-identical, rebuilds and runs `c_smoke`, and runs the unit tests. Run it
after any renderer/backend/ABI change:

```bash
scripts/verify_backends.sh
```

It does **not** replace an external-screenshot check of anything touching
`render()`/`present()`/framebuffer binding specifically — see "The
render-target bug" above for exactly why a same-process capture can't catch
that class of bug, and `docs/SparkGlass_HANDOFF.md` for the verification
checklist to use instead in that case.

## Environment notes for whoever runs this next

- The examples need a real GL/GLES-capable display. This was all developed
  and verified on Linux/Wayland with Mesa.
- `examples/gtk_glarea.rs` and `examples/gtk_glarea_overlay.rs` need GTK4
  dev libraries (`pkg-config gtk4`) and `libepoxy`/`libEGL` at runtime.
- `c_smoke/main.c` needs `gcc` and EGL/GLES2 headers (`EGL/egl.h`,
  `GLES2/gl2.h` — typically `mesa-libEGL-devel`/`libegl1-mesa-dev` or
  equivalent).
- Verifying a window's *actual on-screen output* (not just a same-process
  `glReadPixels` capture — see the render-target bug above) requires an
  external screenshot tool. `spectacle -b -n -a -o <path>` (KDE) was used
  here; substitute whatever is available (`grim` on wlroots compositors,
  `gnome-screenshot`, etc.). This opens real windows and takes real
  screenshots of whatever is on screen — confirm with whoever owns the
  machine before doing this on a shared or actively-used display.
