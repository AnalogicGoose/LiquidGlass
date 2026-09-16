# LiquidGlass

**LiquidGlass** is a cross-platform GPU material engine written in Rust that recreates Apple's **Liquid Glass** visual language for native desktop applications.

The project focuses on one thing above everything else:

> **Visual fidelity is the product. Everything else is infrastructure.**

LiquidGlass aims to provide high-quality glass refraction, blur, distortion, lighting, edge response, and backdrop interaction while remaining independent from any particular UI toolkit.

The long-term goal is a lightweight native library that can be embedded into applications on **Windows** and **Linux** through a stable C-compatible API.

---

## Project Status

LiquidGlass is currently transitioning from a working visual **proof of concept** into a production-oriented rendering library.

The current implementation uses:

- Rust
- `macroquad`
- Raw GLSL shaders
- Signed-distance-field-style geometry
- Multi-pass GPU rendering
- Ping-pong framebuffers

The Macroquad implementation exists primarily as a **visual reference**.

It will remain available while the production renderer is developed so that visual parity can be continuously verified.

Macroquad is **not intended to be part of the final library architecture**.

---

## Vision

LiquidGlass is not intended to become a UI toolkit.

It does not own:

- application windows
- controls
- layouts
- event loops
- navigation
- platform UI
- swapchains
- graphics contexts

Instead, the host application owns its native environment while LiquidGlass renders the visual material.

Conceptually:

```text
                   Host Application
                          │
              ┌───────────┴───────────┐
              │                       │
           Windows                  Linux
              │                       │
           WinUI 3                  GTK 4
              │                       │
       SwapChainPanel             GtkGLArea
              │                       │
            ANGLE               OpenGL / GLES
              │                       │
              └───────────┬───────────┘
                          │
                   Active GL Context
                          │
                          ▼
                    LiquidGlass
                          │
                     Rust Core
                          │
                    GPU Renderer
```

The native UI remains native.

LiquidGlass provides the material underneath or around those controls.

---

# Core Goal

LiquidGlass should eventually compile into a standalone native library:

```text
Windows → .dll
Linux   → .so
```

with a stable:

```c
extern "C"
```

interface.

This allows LiquidGlass to be used from Rust or other languages and frameworks without exposing Rust implementation details across the ABI boundary.

---

# Visual Fidelity Comes First

Architectural cleanliness, portability, and performance are important.

However, none of them justify visibly degrading the effect.

If a change produces a cleaner architecture but makes the glass visibly worse, the architecture must be reconsidered.

The current proof of concept should therefore act as a visual oracle during the migration.

Important characteristics include:

- backdrop-aware blur
- refraction
- lens distortion
- SDF-driven glass geometry
- smooth corner transitions
- edge lighting
- highlights
- chromatic behavior where appropriate
- physically convincing transparency
- interaction with nearby visual content
- smooth animation
- high-DPI rendering
- stable GPU performance

LiquidGlass should never be reduced to:

```text
blur + transparency + rounded rectangle
```

That is not the objective of this project.

---

# Architectural Direction

The current architectural direction is:

```text
Application / UI
       │
       ▼
LiquidGlass C ABI
       │
       ▼
Semantic Scene Model
       │
       ▼
Render Planning
       │
       ▼
OpenGL / GLES Renderer
       │
       ▼
glow
       │
       ▼
GPU
```

The first production renderer is expected to use **OpenGL / OpenGL ES through `glow`**.

The public material model should not depend directly on OpenGL types.

This keeps the architecture open to future backends without prematurely implementing a large generic graphics abstraction.

---

# Scene Model

LiquidGlass is being designed as a **scene-aware material renderer**, rather than a collection of independent glass shaders.

The conceptual model is moving toward:

```text
GlassScene
│
├── Backdrop
│
├── GlassContainer
│   ├── GlassElement
│   ├── GlassElement
│   └── GlassElement
│
└── GlassContainer
    ├── GlassElement
    └── GlassElement
```

A `GlassElement` describes visual intent:

```text
position
size
shape
corner geometry
material
transform
opacity
```

A `GlassContainer` groups related glass surfaces.

Grouping allows the renderer to potentially:

- share backdrop sampling
- reuse blur results
- combine SDF geometry
- optimize rendering passes
- correctly handle nearby glass surfaces
- support future morphing and interaction
- reduce redundant GPU work

The host describes **what should appear**.

LiquidGlass decides **how it should be rendered**.

---

# Rendering Pipeline

The current proof of concept uses a strict multi-pass rendering pipeline.

Conceptually:

```text
Backdrop
   │
   ▼
Geometry / SDF
   │
   ▼
Mask Generation
   │
   ▼
Blur / Filtering
   │
   ▼
Refraction / Distortion
   │
   ▼
Lighting / Edge Response
   │
   ▼
Composite
   │
   ▼
Host Render Target
```

Internally, multiple framebuffers may be used:

```text
Framebuffer A
     ↕
Framebuffer B
```

for ping-pong accumulation.

The exact number of passes, shader programs, framebuffer operations, and intermediate resources are implementation details.

Consumers of LiquidGlass should not need to understand them.

---

# Backdrop Rendering

Glass requires information about what exists behind it.

On platforms such as Apple's own UI frameworks, the system compositor already has this information.

LiquidGlass does not own the Windows or Linux compositor, so the host application must provide the relevant backdrop.

The preferred architecture is GPU-to-GPU:

```text
Host GPU Texture
       │
       ▼
LiquidGlass
       │
       ▼
GPU shader passes
       │
       ▼
Glass output
```

Normal rendering should avoid:

```text
GPU
 ↓
CPU
 ↓
GPU
```

There should be **no CPU readback in the normal rendering path**.

---

# Texture Model

External textures remain owned by the host.

LiquidGlass may sample them but must not destroy them unless ownership has explicitly been transferred.

The higher-level scene API should eventually operate on LiquidGlass resource handles rather than exposing raw graphics API handles everywhere.

Conceptually:

```text
Native GL Texture
       │
       ▼
Import
       │
       ▼
LGTextureHandle
       │
       ▼
GlassScene
```

This keeps the semantic API independent from the first graphics backend.

Future resource import mechanisms could potentially support other graphics APIs without redesigning every material structure.

---

# OpenGL / GLES

The current first-backend direction is:

```text
LiquidGlass
    ↓
  glow
    ↓
OpenGL / OpenGL ES
```

The current baseline target is approximately:

```text
OpenGL ES 3.0+
```

This provides a useful compatibility target for both Linux and ANGLE-based Windows rendering.

The renderer should use newer capabilities when beneficial and available, but core visual behavior should not casually depend on platform-specific features.

---

# Windows

The intended Windows architecture is:

```text
WinUI 3
   │
   ▼
SwapChainPanel
   │
   ▼
ANGLE / EGL
   │
   ▼
OpenGL ES
   │
   ▼
LiquidGlass
   │
   ▼
Direct3D / Vulkan backend selected by ANGLE
```

LiquidGlass itself should not need to know that ANGLE exists.

From LiquidGlass's perspective, it receives a valid OpenGL ES environment from the host.

This isolates platform-specific DXGI, XAML, and ANGLE integration from the material renderer.

---

# Linux

The intended Linux architecture is:

```text
GTK 4
   │
   ▼
GtkGLArea
   │
   ▼
Host OpenGL Context
   │
   ▼
LiquidGlass
```

GTK remains responsible for:

- the application window
- widgets
- events
- layout
- presentation
- native composition

LiquidGlass only performs the glass rendering while the required graphics context is active.

---

# Vulkan

Vulkan is strategically interesting, but it is **not the immediate implementation target**.

The intended philosophy is:

> LiquidGlass is a GPU material engine whose first production backend happens to be OpenGL / GLES.

The internal material and scene models should therefore avoid unnecessary dependency on `glow` or OpenGL-specific types.

However, the project should also avoid premature abstraction.

We are not building a large Vulkan/Metal/OpenGL backend interface before a second real backend exists.

The expected progression is:

```text
Today

LiquidGlass
    ↓
GL/GLES renderer
    ↓
glow


Possible Future

LiquidGlass
     │
 ┌───┴────┐
 ▼        ▼
GLES    Vulkan
 │        │
glow     ash
```

On Windows, ANGLE may also provide Vulkan underneath the GLES interface without requiring LiquidGlass itself to maintain a native Vulkan renderer.

---

# Graphics Context Ownership

The **host application owns the graphics context**.

LiquidGlass should not normally create:

```text
window
GL context
EGL context
swapchain
event loop
presentation loop
```

The expected lifecycle is conceptually:

```text
Host creates graphics environment
          │
          ▼
Host makes context current
          │
          ▼
LiquidGlass initializes GPU resources
          │
          ▼
Host render callback
          │
          ▼
LiquidGlass renders
          │
          ▼
Host presents
```

This model maps naturally to both GTK and ANGLE.

---

# Threading

OpenGL contexts are thread-affine.

LiquidGlass rendering should therefore execute synchronously on the host thread where the appropriate graphics context is current.

The default architecture should **not** create an internal rendering thread.

Avoid:

```text
Host UI Thread
      │
      ▼
LiquidGlass queue
      │
      ▼
Internal Render Thread
      │
      ▼
GL Context
```

unless a future backend explicitly requires and benefits from such a design.

CPU-side preparation may eventually become asynchronous.

Actual GPU commands must obey graphics-context ownership rules.

---

# Native C ABI

The production library will expose an opaque C-compatible API.

Conceptually:

```c
typedef struct LiquidGlassContext LiquidGlassContext;
```

The implementation remains private in Rust.

The C boundary must not expose:

- Rust references
- Rust traits
- Rust generics
- `Vec<T>`
- slices
- `String`
- `dyn Trait`
- Rust enum layout assumptions
- implementation-specific renderer objects

FFI-facing structures should use deliberately defined C-compatible layouts.

**Status:** a v0 of this boundary is implemented at `src/ffi/` (`context.rs`,
`frame.rs`, `error.rs`): `lg_create` / `lg_destroy` / `lg_resize` /
`lg_set_backdrop_gl_texture` / `lg_render_frame` / `lg_present` /
`lg_last_error`, all `extern "C"`, all panic-safe (`catch_unwind` at every
boundary, no unwind ever crosses it), all operating on the opaque
`LiquidGlassContext` pointer plus POD `LGFrame`/`LGGlassElement` structs.
`examples/ffi_smoke.rs` drives the renderer through these functions only — no
`GlGlassRenderer`, no `GlassScene` — and its captured output is byte-identical
to `examples/sandbox.rs`'s direct-API output, confirming the wrapper adds no
behavioral divergence. Not yet done: a `cdylib` build target for an actual
external C/C++ consumer, and container/interaction fields on `LGGlassElement`
(both OPEN elsewhere in this doc).

---

# Frame Submission

The preferred rendering model is **batched frame submission**.

Preferred:

```c
lg_render_frame(context, &frame);
```

rather than:

```c
lg_draw_glass(context, &element1);
lg_draw_glass(context, &element2);
lg_draw_glass(context, &element3);
```

Giving LiquidGlass the full frame allows the renderer to optimize globally.

For example:

```text
Glass A ─┐
Glass B ─┼─ shared backdrop / blur work
Glass C ─┘
```

instead of independently recomputing the same effects.

---

# Coordinates and DPI

The host UI toolkit may work in logical coordinates while the GPU renders in physical pixels.

For example:

```text
Logical UI
800 × 600

Scale
2.0

Framebuffer
1600 × 1200
```

LiquidGlass should receive enough information to convert between those spaces without becoming coupled to GTK or WinUI's DPI systems.

The exact ABI representation is still being designed.

---

# Alpha Composition

LiquidGlass must coexist with native UI controls rendered by the host toolkit.

Conceptually:

```text
┌─────────────────────────────────┐
│ Native controls                 │
│                                 │
│      [ Play ] [ Search ]        │
│                                 │
│ ───── LiquidGlass surface ───── │
│                                 │
└─────────────────────────────────┘
```

Hardware composition and alpha behavior are therefore first-class architectural concerns.

The production implementation must use a clearly documented alpha contract.

---

# Repository Structure

The current migration direction is to keep the project in a **single repository**.

A possible structure is:

```text
LiquidGlass/
│
├── README.md
│
├── Cargo.toml
│
├── docs/
│   └── LiquidGlass_MASTER_ARCHITECTURE.md
│
├── crates/
│   ├── liquidglass-core/
│   ├── liquidglass-gl/
│   └── liquidglass-ffi/
│
├── examples/
│   └── sandbox/
│
├── reference/
│   └── macroquad-poc/
│
└── shaders/
    ├── glass.frag
    ├── glass_mask.frag
    └── blur.frag
```

This layout is provisional.

The repository should only be split into additional crates when the separation provides a practical architectural benefit.

---

# Reference Implementation

The Macroquad PoC should remain available during the migration.

It serves as the visual comparison target.

Recommended development flow:

```text
Macroquad PoC
      │
      │ visual reference
      ▼
New GL Renderer
      │
      ▼
Side-by-Side Comparison
      │
      ▼
Visual Parity
      │
      ▼
PoC becomes reference-only
```

Do not delete the reference implementation simply because the new renderer compiles.

The new renderer must first prove that it preserves or improves the visual result.

---

# Local Sandbox

The production library should remain windowless.

Development still needs a convenient environment for testing.

The local sandbox uses:

```text
winit
  +
glutin
  +
LiquidGlass
```

**Status:** implemented at `examples/sandbox.rs`, backed by the `glow` renderer in
`src/backend/gl/`. Run it with `cargo run --example sandbox`. It renders the same
`glass.frag` / `glass_mask.frag` / `blur.frag` shaders as the Macroquad reference,
through the same multi-pass pipeline, with no Macroquad dependency in the binary.
Visual parity with the reference has been confirmed by side-by-side capture (set
`SPARK_GLASS_SANDBOX_CAPTURE=<path>` to dump a PNG after a few frames, for
comparison against `SPARK_GLASS_CAPTURE` on the reference binary — both are
debug/test-tooling readbacks, never used on the normal render path).

This executable can test:

- shaders
- resizing
- textures
- framebuffer behavior
- materials
- SDF geometry
- DPI scaling
- animations
- GPU performance
- visual regressions

without requiring a full GTK or WinUI application.

---

# Performance Principles

LiquidGlass should prefer:

- GPU-resident resources
- batched rendering
- reused intermediate textures
- persistent GPU resources
- minimal allocation during rendering
- reduced redundant blur passes
- shared container processing
- texture reuse
- minimal driver synchronization

LiquidGlass should avoid:

- CPU readback
- per-frame JSON parsing
- unnecessary GPU resource recreation
- uploading unchanged textures every frame
- one complete blur pipeline per control
- unnecessary `glGet*` state queries
- blocking synchronization where avoidable

---

# Non-Goals

LiquidGlass is not intended to become:

- a full UI toolkit
- a window manager
- a layout engine
- an event system
- a game engine
- a GTK wrapper
- a WinUI wrapper
- an ANGLE wrapper
- a general-purpose graphics engine
- a clone of Apple's private implementation

The goal is narrower:

> Provide an exceptional Liquid Glass-style material renderer that native applications can embed.

---

# Design Philosophy

### 1. Visual quality first

If an optimization significantly harms the material, it is not an acceptable optimization.

### 2. Describe intent, not GPU commands

Applications should describe glass elements and materials.

They should not manually orchestrate LiquidGlass's internal render passes.

### 3. Keep platform ownership outside the renderer

GTK, WinUI, ANGLE, EGL, windows, and event loops belong to the host.

### 4. Keep rendering GPU-resident

Avoid CPU round-trips in the normal pipeline.

### 5. Preserve future flexibility without speculative engineering

Do not allow GL implementation details to unnecessarily infect the entire architecture.

Also do not build unused Vulkan/Metal abstractions merely because they may someday exist.

### 6. Measure before optimizing

Render-state restoration, framebuffer strategies, blur implementations, batching behavior, and backend choices must eventually be benchmarked on real hardware.

### 7. The current effect is evidence

The existing PoC already demonstrates the desired visual direction.

Migration must preserve what works rather than restarting from assumptions.

---

# Current Architectural Decisions

| Area | Direction |
|---|---|
| Language | Rust |
| Production graphics API | OpenGL / GLES first |
| GL wrapper | `glow` |
| Approximate baseline | GLES 3.0+ |
| Window ownership | Host |
| GL context ownership | Host |
| UI ownership | Host |
| Render thread | Host graphics thread |
| Production event loop | None |
| ABI | `extern "C"` |
| Context exposure | Opaque handle |
| Per-frame description | C-compatible structures |
| Submission | Batched |
| Normal CPU readbacks | None |
| External textures | GPU-first |
| Windows | WinUI 3 + ANGLE |
| Linux | GTK 4 + GtkGLArea |
| Sandbox | `winit` + `glutin` |
| Macroquad | Visual reference only |
| Vulkan | Possible future native backend |
| Native controls | Host-rendered |

---

# Still Open

Several architectural questions remain intentionally unresolved.

These should be discussed and tested before being frozen.

### Render Target Contract

Should LiquidGlass:

- render into the currently bound framebuffer;
- accept a host-provided framebuffer;
- create its own output texture;
- support multiple target modes?

### OpenGL State Isolation

Should LiquidGlass:

- preserve all relevant GL state;
- preserve a documented subset;
- provide safe/fast modes;
- require the host adapter to restore state?

This must be decided using real performance measurements.

### Backdrop Scope

Should a scene have:

```text
one global backdrop
```

or should different containers be able to reference different backdrops?

### Resource Lifecycle

The library must define behavior when:

- GL contexts are recreated;
- windows resize;
- framebuffer dimensions change;
- a device becomes invalid;
- external textures disappear;
- GtkGLArea is unrealized/recreated;
- ANGLE recreates surfaces.

### Material Model

The exact split between:

```text
per-scene
per-container
per-material
per-element
```

parameters remains open.

---

# Development Roadmap

## Phase 0 — Preserve the Reference

Freeze/tag the current visual PoC.

Example:

```text
v0.1-poc-reference
```

---

## Phase 1 — Semantic Model

Define:

```text
Scene
Backdrop
Container
GlassElement
Material
TextureHandle
RenderTarget
```

without immediately writing the complete FFI.

---

## Phase 2 — C ABI

Define:

- handles
- ownership
- lifetime rules
- error model
- versioning
- frame structures
- resource APIs
- thread requirements

**Status:** v0 implemented — see the "Native C ABI" section above for what
exists and what's still missing (`cdylib` output, container/interaction
fields).

---

## Phase 3 — GL Renderer

Replace Macroquad's rendering responsibilities with:

```text
glow
```

while reproducing the existing visual pipeline.

**Status:** implemented — `src/backend/gl/` (`renderer.rs`, `shader.rs`, `target.rs`,
`texture.rs`). Reuses the reference fragment shaders verbatim; only the vertex
stage and host-facing plumbing differ, per the "shaders are the material, not
the plumbing" rule. Not yet done: the C ABI (Phase 2) and GL state-preservation
contract (still an open question below) — this phase only covers the renderer
itself running host-side, in Rust, against a `glow::Context`.

---

## Phase 4 — Standalone Sandbox

Build:

```text
winit + glutin + LiquidGlass
```

and establish visual parity with the PoC.

**Status:** implemented — `examples/sandbox.rs`. Visual parity with the Macroquad
reference confirmed by capture comparison at 1280×800 with the "Clear" profile.

---

## Phase 5 — Linux Integration

Validate:

```text
GTK 4 + GtkGLArea + LiquidGlass
```

---

## Phase 6 — Windows Integration

Validate:

```text
WinUI 3 + ANGLE + LiquidGlass
```

---

## Phase 7 — Real Application Validation

The first major real-world consumer is expected to be **GoosicReborn**.

The architecture succeeds if GoosicReborn can use essentially the same LiquidGlass material API on both platforms while only its platform integration layer changes.

---

## Phase 8 — Advanced Fidelity

Investigate:

- shared SDF fields
- optimized backdrop regions
- better lens simulation
- dynamic luminance adaptation
- highlight behavior
- chromatic dispersion
- material interaction
- morphing
- animation
- shared blur caches
- advanced GPU reduction/compute techniques

---

## Phase 9 — Future Backends

Only after the semantic architecture proves itself should a native second graphics backend be seriously evaluated.

Potential example:

```text
Vulkan
```

The existence of that future possibility should influence clean boundaries today, but it should **not dominate the implementation today**.

---

# AI / Contributor Guidance

When working on LiquidGlass:

## DO

- preserve visual fidelity
- preserve the PoC until parity exists
- reason about GPU ownership carefully
- keep render passes internal
- keep UI ownership in the host
- maintain zero normal CPU readback
- consider batching and backdrop reuse
- distinguish semantic API from graphics backend
- benchmark architectural performance decisions
- document ownership and lifetime explicitly
- keep platform-specific integration isolated

## DO NOT

- replace the material with simple blur
- delete working visual behavior because a cleaner abstraction is easier
- introduce CPU screenshots into the normal render path
- make LiquidGlass a UI toolkit
- expose Rust-specific types through C
- make GTK/WinUI knowledge part of the material core
- make the public scene model depend directly on `glow`
- prematurely rewrite the project around Vulkan
- design dozens of speculative backend traits
- rebuild GPU resources every frame unnecessarily
- expose ping-pong passes to applications
- assume compilation equals visual success

---

# The Rule

When there is uncertainty, return to this:

> **LiquidGlass exists to render exceptional glass.**

The architecture, ABI, backend, platform integrations, optimizations, and resource systems exist to make that possible.

**Visual fidelity is the product. Everything else is infrastructure.**
