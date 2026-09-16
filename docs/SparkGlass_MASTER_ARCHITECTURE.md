# SparkGlass — Master Architecture, Rendering & AI Guidance

> **Status:** Architecture / feature-freeze discussion document  
> **Project:** `SparkGlass`  
> **Language:** Rust  
> **Primary platforms:** Windows + Linux  
> **Primary consumer target:** GoosicReborn  
> **Current implementation status:** Early production engine (`glow` renderer, C ABI, and Linux/GTK4 integration implemented and verified — see `docs/SparkGlass_IMPLEMENTATION_STATUS.md`). The Macroquad implementation described below is preserved as an internal visual reference, not the project itself.  
> **Most important project requirement:** **VISUAL FIDELITY TO APPLE'S LIQUID GLASS**

---

# 0. READ THIS FIRST — THE NORTH STAR

## 🔴 Priority #1: the material must look right

SparkGlass exists to reproduce the **visual behavior, optical character, motion, layering, refraction, blur, edge response, lighting, geometry, and fluidity** of Apple's Liquid Glass as faithfully as reasonably possible on Windows and Linux.

Everything else exists to support that objective.

The priority order is:

```text
1. VISUAL FIDELITY
   ↓
2. Correct GPU/compositing behavior
   ↓
3. Stable architecture and ownership rules
   ↓
4. Performance
   ↓
5. Cross-platform ergonomics
   ↓
6. API elegance / abstraction purity
```

This does **not** mean performance is unimportant. Liquid Glass must still render interactively and efficiently. It means that an optimization which noticeably breaks the material is not considered a success.

> [!IMPORTANT]
> If a future AI proposes a simpler architecture that loses the optical behavior of the current PoC, **reject that architecture unless visual parity can be preserved**.

> [!IMPORTANT]
> Do not replace the existing shader behavior merely because a different implementation is more idiomatic, cleaner, or easier to maintain. Preserve the current visual reference until the replacement has been proven visually equivalent or better.

---

# 1. PURPOSE OF THIS DOCUMENT

This file is the **master architectural context** for humans and AI agents working on SparkGlass.

It exists so that a future contributor does not accidentally:

- turn SparkGlass into a UI toolkit;
- couple the core to GTK, WinUI, ANGLE, `winit`, or `glutin`;
- replace the visual renderer with a simpler but visibly inferior approximation;
- expose raw Rust types across the ABI;
- treat the current Macroquad PoC architecture as production architecture;
- add CPU readback to the normal rendering path;
- design the entire API around raw OpenGL object names;
- prematurely rewrite everything around Vulkan;
- create a huge generic GPU abstraction before a second backend exists;
- ignore Apple's grouping/container behavior;
- model each glass object as an isolated rectangle when shared backdrop/SDF behavior matters;
- rewrite the shaders before understanding what visual properties they currently preserve.

This document distinguishes four kinds of statements:

| Marker | Meaning |
|---|---|
| **FROZEN** | Core architectural principle. Do not change casually. |
| **PROVISIONAL** | Strong current direction, but still open to evidence. |
| **OPEN** | Deliberately unresolved. Must be decided before the relevant implementation phase. |
| **FUTURE** | Supported by the architecture conceptually, but not a current implementation target. |

---

# 2. PROJECT OVERVIEW

SparkGlass is a cross-platform GPU visual-material library written in **Rust**.

The long-term output is intended to be a native standalone library:

```text
Windows: SparkGlass.dll
Linux:   libsparkglass.so
```

with a stable **C-compatible ABI** exposed using Rust `extern "C"` functions.

The first major application intended to consume SparkGlass is:

**GoosicReborn**  
https://github.com/AnalogicGoose/GoosicReborn/tree/development

However, SparkGlass must remain independently usable by other native desktop applications.

---

# 3. CURRENT PoC

The current proof of concept uses:

- Rust
- `macroquad`
- raw GLSL shaders
- `glass.frag`
- `glass_mask.frag`
- `blur.frag`
- multi-pass rendering
- ping-pong framebuffers
- SDF-style geometry / masking behavior

The PoC accurately reproduces the desired Apple/Figma Liquid Glass appearance and is therefore an **important visual reference**.

## FROZEN

Macroquad is **not** part of the desired production architecture.

Macroquad currently provides things that the final library must not own:

```text
Macroquad
 ├── window creation
 ├── graphics context
 ├── event loop
 ├── presentation
 └── rendering abstraction
```

The production design must separate those responsibilities.

## DO NOT

Do not delete the PoC immediately during migration.

Keep it available as a visual baseline until the new renderer reproduces its output.

---

# 4. THE CORE ARCHITECTURAL CONTRACT

## FROZEN

> **The host application owns the platform. SparkGlass owns the glass material rendering.**

The host owns:

- application window;
- UI toolkit;
- event loop;
- platform graphics context;
- graphics-context creation;
- presentation / swapchain;
- application layout;
- native controls;
- platform-specific compositor integration.

SparkGlass owns:

- glass geometry interpretation;
- SDF generation / evaluation strategy;
- internal shaders;
- blur;
- refraction/lensing;
- chromatic/dispersion effects where applicable;
- tinting;
- edge/specular lighting;
- glass shadows;
- internal render passes;
- ping-pong or other temporary render targets;
- batching/group optimization;
- material implementation;
- GPU resources internal to the renderer.

SparkGlass must **not** own:

```text
window
widget tree
navigation
button semantics
menu semantics
event loop
application swapchain lifecycle
platform-native view hierarchy
```

---

# 5. TARGET PLATFORM ARCHITECTURE

## 5.1 Windows

Target integration:

```text
WinUI 3
   ↓
XAML SwapChainPanel
   ↓
platform adapter
   ↓
ANGLE / EGL
   ↓
OpenGL ES 3.x
   ↓
ANGLE backing renderer
   ├── D3D11
   └── potentially Vulkan
   ↓
DXGI / Windows GPU stack
```

### FROZEN

SparkGlass itself should **not need to know that ANGLE exists**.

From the renderer's perspective it should see a usable OpenGL ES environment.

This preserves a clean separation:

```text
SparkGlass = material renderer
ANGLE       = API translation/platform layer
WinUI       = native UI/composition layer
```

ANGLE itself supports GLES translation to multiple backends, including D3D11 and Vulkan. Therefore SparkGlass can potentially benefit from Vulkan underneath ANGLE without becoming a Vulkan renderer itself.

---

## 5.2 Linux

Target integration:

```text
GTK 4
  ↓
GtkGLArea
  ↓
GdkGLContext
  ↓
current OpenGL / GLES context
  ↓
SparkGlass
```

GTK documents that `GtkGLArea`:

- creates/owns a `GdkGLContext`;
- creates a framebuffer for the widget;
- makes that framebuffer the default target during rendering;
- calls the render callback with the GL context current;
- integrates the completed rendering into GTK's larger scene graph as a texture.

This is almost exactly the embedding model SparkGlass needs.

### FROZEN

SparkGlass should operate **inside the host's GL render callback**.

It should not create a second unrelated context behind GTK's back.

---

# 6. PRODUCTION DEPENDENCY PHILOSOPHY

## PROVISIONAL — backend #1

```text
Rust
 ↓
glow
 ↓
OpenGL / OpenGL ES
```

The production core should not require:

- Macroquad
- `winit`
- `glutin`
- GTK
- WinUI
- ANGLE-specific APIs

Those dependencies belong outside the production material core.

---

# 7. WHY `glow` IS CURRENTLY THE PREFERRED GL LAYER

Candidates discussed:

- `glow`
- generated `gl` bindings
- Khronos registry/binding crates
- explicit EGL bindings

## PROVISIONAL RECOMMENDATION: `glow`

Why it fits SparkGlass:

- supports OpenGL and OpenGL ES style environments;
- accepts function loading supplied by the host;
- has no requirement to own a window;
- has no requirement to create the graphics context;
- is thin enough that SparkGlass remains close to GL;
- is more ergonomic than raw generated global bindings;
- is a good fit for GLES on ANGLE and GL/GLES on Linux.

## Why raw `gl` is not currently preferred

Raw generated bindings are valid and predictable, but provide less of a portability-oriented wrapper around GL/GLES behavior.

They remain a possible fallback if `glow` proves to impose a concrete limitation.

## Why EGL should not automatically be a core dependency

EGL is useful on Windows/ANGLE and other platforms, but context creation is a **host/platform-adapter responsibility**.

SparkGlass should not require EGL merely because one host integration uses it.

---

# 8. GRAPHICS BASELINE

## PROVISIONAL

The renderer should target a **GLES 3.0-compatible baseline** wherever practical.

Conceptually:

```text
SparkGlass shader/rendering contract
             ↓
      GLES 3.0-compatible subset
         ┌───┴────┐
         ↓        ↓
       ANGLE    Desktop GL/GLES
```

Why:

- Windows/ANGLE is naturally GLES-oriented;
- reduces shader/API divergence;
- keeps the renderer portable;
- enables ANGLE to choose D3D11 or Vulkan underneath;
- avoids making desktop GL-only features foundational.

## OPEN

We still need to define:

- strict minimum GLSL ES version;
- mandatory texture formats;
- mandatory framebuffer formats;
- whether half-float render targets are required;
- extension policy;
- whether optional higher-quality paths use GLES 3.1+ features.

### Rule

Optional GPU capabilities may improve quality/performance, but the capability system must not silently produce a materially different visual identity.

---

# 9. VULKAN STRATEGY

## Key conclusion

**Vulkan matters to the architecture, but native Vulkan should not be the first backend.**

## FUTURE VISION

SparkGlass should conceptually be:

> **a GPU material engine whose first renderer is GL/GLES**

not:

> a permanently GL-specific public API.

The desired long-term relationship is:

```text
             SparkGlass semantic model
                       │
                       ▼
                  Render plan
                       │
             ┌─────────┴─────────┐
             ▼                   ▼
       GL/GLES backend       Vulkan backend
           TODAY                FUTURE
             │                   │
            glow                ash / equivalent
```

## Why Vulkan is attractive later

Vulkan offers:

- explicit memory ownership;
- explicit synchronization;
- explicit image layouts;
- command buffers;
- compute-shader workflows;
- lower API call overhead in some workloads;
- more deliberate external-memory/image sharing;
- stronger opportunities for complex GPU-only pipelines.

These could be valuable for:

- multi-stage blur;
- luminance reductions;
- complex shared SDF processing;
- large scenes;
- GPU resource sharing;
- compute-driven optimizations.

## Why native Vulkan is NOT the immediate backend

### GTK integration

`GtkGLArea` is a clean supported custom-rendering API.

GTK's public `GdkVulkanContext` was deprecated and GTK explicitly states that it does not expose Vulkan internals. A native Vulkan backend therefore does not currently have an equally elegant `GtkGLArea`-like embedding path.

### Complexity

Vulkan would immediately force us to solve:

- instance/device ownership;
- queues;
- swapchain or external target integration;
- semaphores/fences;
- image layout transitions;
- descriptor management;
- command buffer lifetime;
- allocator strategy;
- platform synchronization.

Those are worthwhile only when a concrete backend need justifies them.

## Important ANGLE observation

ANGLE can translate GLES to **Vulkan**.

Therefore this is possible:

```text
SparkGlass
   ↓
GLES
   ↓
ANGLE
   ↓
Vulkan
   ↓
GPU
```

without changing SparkGlass's first renderer.

This gives us a valuable experimental path on Windows before writing a native Vulkan backend.

---

# 10. DO NOT PREMATURELY BUILD A GIANT GPU ABSTRACTION

We want future backend freedom without creating an abstraction framework for hypothetical APIs.

## BAD

```text
GlassElement
  └── glow::NativeTexture
```

This leaks the GL implementation into the semantic model.

## ALSO BAD — right now

```rust
trait GraphicsBackend {
    // 70+ methods trying to abstract OpenGL, Vulkan, Metal, D3D...
}
```

That is premature architecture.

## GOOD

```text
GlassElement
  └── SGTextureHandle

GL renderer
  └── resolves SGTextureHandle to GL resource
```

And internally:

```text
Material model
      ↓
Render description / render plan
      ↓
GL implementation
```

When backend #2 exists, we will know what the *real* common abstraction needs to be.

---

# 11. APPLE LIQUID GLASS — WHAT WE SHOULD LEARN FROM IT

Apple does not publicly expose the complete private render graph of Liquid Glass.

We must distinguish:

1. Apple's **public documented behavior**;
2. third-party/private reverse-engineering observations;
3. our own architectural inference.

Do not treat private class names as stable Apple API.

---

# 12. APPLE — PUBLICLY DOCUMENTED PRINCIPLES

Apple publicly describes Liquid Glass as a dynamic material that:

- blurs content behind it;
- reflects color/light from surrounding content;
- bends/concentrates light through lensing;
- reacts to interaction;
- adapts to underlying content;
- adapts to element size;
- changes tint, shadow, dynamic range and optical character;
- changes lensing/refraction as the material becomes larger/thicker;
- can pick up ambient color from nearby content.

This is critically important.

## Visual target is NOT simply

```text
background blur
+
transparent white rectangle
```

The target behaves more like a **dynamic optical material**.

---

# 13. APPLE'S CONTAINER MODEL IS HIGHLY RELEVANT

Apple exposes `GlassEffectContainer` in SwiftUI and equivalent container concepts in other frameworks.

Apple documents that grouped glass shapes:

- render together;
- improve rendering performance;
- can interact;
- can blend/merge based on proximity;
- can morph into one another;
- share adaptive appearance;
- require grouping for visual correctness in certain cases.

Apple also explains that glass samples content from an area **larger than the glass element itself** because of refraction/nearby-color behavior.

This has a major implication for SparkGlass.

## OLD simplistic model

```text
Frame
 ├── Element A
 ├── Element B
 └── Element C
```

where every element is completely independent.

## BETTER MODEL

```text
GlassScene
│
├── Backdrop
│
├── GlassContainer A
│    ├── Element A
│    ├── Element B
│    └── Element C
│
└── GlassContainer B
     ├── Element D
     └── Element E
```

The container gives SparkGlass semantic permission to treat nearby elements as a coherent material group.

Possible internal optimizations/behavior:

- shared backdrop sampling;
- shared blur regions;
- shared adaptive luminance;
- merged SDF fields;
- reduced number of passes;
- geometry interaction;
- future morphing;
- future liquid joining/separation.

## PROVISIONAL

Adopt **scene → containers → elements** as the conceptual model rather than treating every glass item as a completely isolated draw call.

---

# 14. THE GUI RAMBO VIDEO — WHAT MATTERS FOR US

Reference:

**Gui Rambo — “Inside Liquid Glass: How iOS 26 Synthesizes Refractive UI”**  
https://www.youtube.com/watch?v=oj20mb8c0yI

The most important value of this video for SparkGlass is not copying private Apple APIs.

The important lesson is architectural:

> Apple's glass appears to be deeply integrated into its compositor/layer rendering system rather than implemented as a single decorative UI shader.

## High-value conclusions for our project

### 14.1 Think compositor/material system, not “one magic fragment shader”

The material should be treated as cooperating stages:

```text
backdrop
   ↓
geometry / SDF
   ↓
blur / scattering
   ↓
refraction / lensing
   ↓
edge response / dispersion
   ↓
specular / highlights
   ↓
shadow / elevation
   ↓
composite
```

The exact pass count remains an internal implementation detail.

### 14.2 Backdrop is first-class

Apple can access the composited scene beneath its own material because Apple controls the compositor.

SparkGlass does **not** control GTK or WinUI's complete scene.

Therefore the host must provide the equivalent information explicitly — normally as a GPU-resident backdrop texture or compatible surface.

### 14.3 Geometry must be first-class

SparkGlass should receive semantic geometry:

- bounds;
- corner radii / shape parameters;
- transforms;
- visibility;
- container membership;
- material properties.

The host should not be required to provide a pre-rendered CPU mask.

### 14.4 SDF is strategically aligned

Private reverse-engineering work around Apple's implementation has reported SDF-oriented internal layer types and pipelines.

Whether Apple's exact private classes change is not important.

What matters for us is that an SDF-style representation is well suited to:

- rounded/superellipse geometry;
- smooth shape union;
- distance-aware edge lighting;
- refraction normals;
- morphing;
- liquid merging;
- scalable geometry without CPU raster masks.

Our existing SDF direction therefore appears strategically sound.

### 14.5 Adaptivity matters

The glass should eventually be able to react to:

- backdrop luminance;
- backdrop color;
- element scale/size;
- nearby color/light;
- interaction/motion.

Any analysis should stay on GPU during production rendering.

No normal-path `glReadPixels()`.

### 14.6 Visual effects may be logically separate layers/stages

A high-fidelity implementation may need independently tunable stages for:

- body distortion;
- blur;
- tint;
- rim response;
- directional specular;
- chromatic dispersion;
- inner/outer edge lighting;
- shadow/elevation.

Do not force all behavior into one shader simply for aesthetic code simplicity.

---

# 15. PRIVATE APPLE REVERSE-ENGINEERING — USE CAREFULLY

Third-party reverse-engineering has reported private Core Animation concepts such as:

- `CABackdropLayer`;
- `CASDFLayer`;
- `CASDFElementLayer`;
- `glassBackground`-style filtering;
- SDF output/highlight stages.

These observations are useful as **evidence about architecture**, not as dependencies or stable API specifications.

## DO

Extract architectural ideas:

```text
backdrop-aware rendering
shared shape representation
SDF geometry
separate edge/highlight stages
compositor-level processing
grouped elements
```

## DO NOT

- depend on private Apple frameworks;
- copy private class names into our public ABI;
- claim Apple's exact implementation is guaranteed;
- design SparkGlass around assumptions that only make sense inside Core Animation.

---

# 16. OUR EQUIVALENT OF APPLE'S COMPOSITOR ACCESS

Apple's system compositor can know what content is behind a glass surface.

SparkGlass cannot assume that.

Therefore the architecture becomes:

```text
HOST
 ├── native scene
 ├── native UI controls
 ├── backdrop GPU texture/surface
 └── glass geometry
           │
           ▼
     SparkGlass Scene
           │
           ▼
       GPU effects
           │
           ▼
       render target
           │
           ▼
   host UI composition
```

This is a necessary platform-boundary difference, not a visual-design difference.

---

# 17. SCENE MODEL

## PROVISIONAL SEMANTIC MODEL

```text
SGScene / SGFrame
│
├── viewport / scale
├── backdrop(s)
├── render target reference/config
│
└── glass containers[]
     │
     ├── container properties
     │    ├── sampling behavior
     │    ├── spacing / merge threshold (future)
     │    └── shared material context
     │
     └── elements[]
          ├── geometry
          ├── transform
          ├── material reference
          ├── opacity
          ├── backdrop reference if needed
          └── interaction state if needed
```

The semantic API tells SparkGlass **what the scene is**.

The renderer decides **how many GPU passes are required**.

---

# 18. IMMEDIATE MODE VS FRAME DESCRIPTION

## NOT PREFERRED

```c
sg_draw_glass(ctx, &a);
sg_draw_glass(ctx, &b);
sg_draw_glass(ctx, &c);
```

This hides the complete scene from SparkGlass and limits optimization.

## PREFERRED

```c
sg_render_frame(ctx, &frame);
```

where `frame` describes all relevant glass containers/elements.

Benefits:

- batching;
- shared blur;
- shared backdrop sampling;
- pass elimination;
- spatial optimization;
- future merged SDF processing;
- future morphing/interaction;
- consistent adaptive material behavior.

## FROZEN PRINCIPLE

> **The host describes intent. SparkGlass decides the render graph.**

The public ABI must never require the host to manually execute:

```text
mask pass
blur horizontal
blur vertical
refraction pass
highlight pass
composite pass
```

Those are internal implementation details.

---

# 19. GEOMETRY MODEL

SparkGlass should understand material geometry, not UI widgets.

Possible `GlassElement` semantic data:

```text
position
size
shape
corner radius / corner radii
superellipse parameters
transform
opacity
material
container membership
backdrop selection
interaction state
```

It should not understand:

```text
Button
Label
List
Menu
NavigationBar
GTKWidget
XAMLControl
```

---

# 20. SDF STRATEGY

## PROVISIONAL

SDF should remain a central internal technique unless measurements or visual evidence show a better approach.

Why SDF fits the project:

- analytic curved shapes;
- smooth corners;
- superellipses;
- edge distance information;
- normal estimation;
- distortion/refraction inputs;
- shape merging;
- morphing;
- resolution independence;
- GPU-friendly evaluation.

Potential container behavior:

```text
Element SDF A ─┐
Element SDF B ─┼──→ combined/group field ─→ material stages
Element SDF C ─┘
```

## OPEN

We still need to determine whether the production implementation uses:

- direct analytic SDF evaluation per fragment;
- an intermediate SDF texture;
- hybrid analytic + cached representation;
- tiled/region-based SDF processing;
- compute-based generation in a future backend.

Choose based on **visual quality + measured GPU cost**, not ideology.

---

# 21. BACKDROP MODEL

Backdrop content is a core material input.

Examples:

```text
album artwork
animated application artwork
video frame
host-rendered background scene
other GPU-rendered content
```

## FROZEN

Normal rendering must avoid:

```text
GPU → CPU → GPU
```

The preferred flow is:

```text
host GPU texture
      ↓
SparkGlass samples texture
      ↓
GPU material pipeline
      ↓
GPU output
```

---

# 22. EXTERNAL TEXTURE OWNERSHIP

## FROZEN OWNERSHIP RULE

> The host owns externally supplied textures. SparkGlass may sample them but must not destroy them.

For GL backend v1, an imported external texture must be valid in the same compatible GL context/share group used by SparkGlass.

Important concerns to specify:

- texture target;
- dimensions;
- format expectations;
- color space;
- origin/orientation;
- alpha mode;
- lifetime;
- synchronization;
- reallocation;
- frame validity;
- context/share group.

## Public scene should NOT directly depend on `GLuint`

Preferred conceptual flow:

```text
GLuint / host GL texture
          ↓
sg_import_gl_texture(...)
          ↓
SGTextureHandle
          ↓
scene/material references SGTextureHandle
```

This makes future backends possible.

Possible future imports:

```text
sg_import_gl_texture(...)
sg_import_vulkan_image(...)
sg_import_d3d_texture(...)
```

without redesigning `GlassElement`.

---

# 23. CPU TEXTURE UPLOADS

CPU → GPU upload is not the same thing as GPU readback.

Occasional texture upload may be acceptable for static or infrequently changing album artwork.

What we want to avoid is doing this every frame unnecessarily:

```text
CPU pixels
   ↓
FFI
   ↓
glTexImage2D
   ↓
GPU
```

Preferred when possible:

```text
existing GPU texture
   ↓
import/reference
   ↓
SparkGlass
```

---

# 24. RENDER TARGET — IMPORTANT OPEN DECISION

One of the most important unresolved contracts is:

> **What exactly does `sg_render_frame()` render into?**

Possible models:

### A — current framebuffer

```text
host binds target FBO
      ↓
sg_render_frame()
```

Advantages:

- simple;
- maps naturally to `GtkGLArea`;
- minimal API surface.

Disadvantages:

- implicit state;
- more fragile integration contract.

### B — explicit host framebuffer

```text
host passes target description
      ↓
SparkGlass binds/render/restores as specified
```

Advantages:

- explicit;
- easier to reason about;
- easier validation.

### C — SparkGlass-owned output texture

```text
SparkGlass renders to own output
      ↓
host composites output texture
```

Advantages:

- strong isolation;
- potentially useful for complex compositor integration.

Disadvantages:

- extra resource lifecycle;
- may complicate zero-copy composition.

### D — support multiple target modes

Potentially the best long-term flexibility, but do not add modes until actual platform needs justify them.

## PROVISIONAL DIRECTION

Keep **scene semantics** independent from **backend render-target mechanics**.

Conceptually:

```text
SGScene
  └── glass semantics

SGGLRenderTarget
  └── framebuffer/backend integration
```

Do not put raw framebuffer implementation detail into material definitions.

---

# 25. ALPHA / COMPOSITING

## PROVISIONAL

Prefer a well-documented **premultiplied-alpha** output contract unless platform experiments demonstrate a different requirement.

Why this matters:

- native UI will be composed above/beside SparkGlass;
- transparency edges must not produce dark halos;
- blur/refraction output must composite predictably.

The exact blend state and color-space rules must be tested on both GTK and WinUI.

---

# 26. COLOR SPACE / HDR — DO NOT IGNORE FOREVER

## OPEN / LATER

Visual fidelity can be destroyed by incorrect color-space assumptions.

Eventually define:

- sRGB vs linear sampling;
- where blur occurs;
- where tint occurs;
- framebuffer color space;
- premultiplied-alpha space;
- HDR/extended-range behavior;
- texture transfer functions.

## Rule

Do not silently assume “RGBA8 everywhere” is visually correct merely because it works technically.

---

# 27. CONTEXT OWNERSHIP

## FROZEN

> **The host creates and owns the GL/GLES context. SparkGlass uses the context that is current.**

SparkGlass should not expose production APIs such as:

```text
create_window()
create_swapchain()
create_gl_context()
present()
```

Those belong to the host/platform adapter.

---

# 28. THREADING MODEL

## FROZEN

GL-touching SparkGlass operations must run synchronously on the host thread where the correct GL context is current.

Preferred flow:

```text
Host UI/render thread
        │
        │ context current
        ▼
sg_render_frame(...)
        │
        ▼
SparkGlass issues GPU commands
        │
        ▼
return to host
```

Do **not** add an internal render thread by default.

Why:

- context affinity;
- synchronization complexity;
- texture sharing complexity;
- additional latency;
- difficult lifecycle handling;
- less predictable host integration.

## PROVISIONAL DEBUG SAFETY

The Rust context may record its GL owner thread and, in debug builds, report an error when GL functions are called from the wrong thread.

---

# 29. CONTEXT / RESOURCE LIFECYCLE

A robust lifecycle likely needs to distinguish CPU-side state from GPU-side state.

Conceptual lifecycle:

```text
sg_create()
   ↓
Rust-side context exists
   ↓
host makes GL context current
   ↓
sg_initialize_gl(...)
   ↓
GPU resources created
   ↓
sg_render_frame(...)
   ↓
...
   ↓
host makes correct GL context current
   ↓
sg_release_gl_resources(...)
   ↓
host destroys/recreates platform GL context if needed
   ↓
possibly sg_initialize_gl(...) again
   ↓
sg_destroy()
```

## OPEN

Exact names/signatures are not frozen.

## Must handle

- GTK `realize` / `unrealize`;
- WinUI surface recreation;
- context loss;
- framebuffer resize;
- DPI changes;
- context recreation;
- GPU resource invalidation;
- imported texture invalidation.

---

# 30. FFI BOUNDARY

## FROZEN PRINCIPLE

The C ABI must not expose Rust implementation details.

Do not expose:

- Rust references;
- Rust slices;
- `Vec<T>`;
- Rust generics;
- Rust traits;
- `dyn Trait`;
- Rust-owned strings;
- layout-unstable Rust enums/structs;
- `glow` types.

Preferred opaque type:

```c
typedef struct SparkGlassContext SparkGlassContext;
```

Internally Rust may hold any implementation it wants.

Externally the host sees only an opaque handle/pointer.

---

# 31. FFI DATA MODEL

## PROVISIONAL

Use ABI-safe C/POD structs and pointer+count arrays for hot-path frame submission.

Examples of semantic POD structures:

```text
SGRect
SGTransform
SGColor
SGGlassElement
SGGlassContainer
SGFrame
SGRenderTarget
```

These names and layouts are **not final**.

## Avoid JSON in the render hot path

JSON would add:

- parsing;
- allocations;
- strings;
- larger memory traffic;
- weaker compile-time host bindings;
- unnecessary frame latency.

JSON may still be useful for:

- tools;
- debug captures;
- presets;
- offline material definitions;
- test fixtures.

---

# 32. ABI VERSIONING

The ABI must survive Rust compiler/library changes.

Potential techniques:

- ABI version query;
- `struct_size` fields;
- explicit integer enum representations;
- reserved fields;
- capability queries;
- opaque handles;
- no Rust layout assumptions.

Example conceptual pattern:

```text
SGFrame
 ├── struct_size
 ├── version
 ├── flags
 └── ...
```

## Rule

Do not expose an ABI field just because an internal shader currently needs it.

Expose **material meaning**, not temporary implementation plumbing.

---

# 33. ERROR HANDLING

The C ABI needs predictable non-panicking behavior.

Requirements:

- no Rust panic may unwind across C ABI;
- errors should have stable numeric codes;
- optional diagnostic message retrieval can exist;
- wrong-thread/context errors should be diagnosable;
- shader compilation/link failures must be reportable;
- unsupported capabilities must be reportable.

Exact mechanism is still OPEN.

---

# 34. MATERIAL API

The public material API should describe perceptual behavior, not GLSL uniforms unless the uniform directly represents a stable material concept.

Possible stable concepts:

- material variant;
- tint;
- opacity;
- refraction strength;
- distortion strength;
- blur/scattering strength;
- edge/specular intensity;
- chromatic dispersion;
- elevation/shadow response;
- interaction response.

Possible geometry concepts:

- corner radius;
- superellipse shape;
- shape family;
- transform.

## OPEN

Determine which properties are:

```text
per-context
per-material
per-container
per-element
per-frame
```

Do not freeze this until the current PoC shader parameters are mapped to perceptual/material meaning.

---

# 35. RENDER PIPELINE

The current PoC uses strict multi-pass accumulation with ping-pong framebuffers.

Conceptually:

```text
Backdrop Texture
      ↓
geometry / mask / SDF stage
      ↓
intermediate target A
      ↓
blur / optical stage
      ↓
intermediate target B
      ↓
additional optical stages
      ↓
composite
      ↓
Host Render Target
```

## FROZEN PRINCIPLE

The host must not know:

- number of passes;
- ping vs pong;
- intermediate texture count;
- shader filenames;
- exact blur algorithm;
- whether future implementation uses fragment or compute shaders.

Those are private renderer implementation details.

---

# 36. POTENTIAL INTERNAL RENDER GRAPH

Do not treat this as frozen implementation.

A fidelity-oriented internal plan may resemble:

```text
                     SGScene
                        │
                        ▼
                 Geometry Stage
                        │
                  SDF / shape data
                        │
          ┌─────────────┴─────────────┐
          ▼                           ▼
   Backdrop sampling             Material context
          │                           │
          ├─────────────┬─────────────┤
          │             │             │
          ▼             ▼             ▼
        Blur       Adaptivity      Refraction
          │             │             │
          └─────────────┴─────────────┘
                        │
                        ▼
               Dispersion / lens
                        │
                        ▼
                Rim / specular
                        │
                        ▼
                 Shadow / depth
                        │
                        ▼
                    Composite
```

The real render graph must be based on measured needs of the existing visual effect.

---

# 37. GL STATE OWNERSHIP

SparkGlass is embedded in another renderer, so it cannot casually corrupt global GL state.

Potential state touched:

- framebuffer binding;
- viewport;
- scissor;
- program;
- VAO;
- VBO/IBO;
- active texture;
- texture bindings;
- blending;
- depth;
- stencil;
- pixel-store state;
- color masks.

## OPEN — important performance decision

### Option A — preserve everything

SparkGlass queries/saves/restores relevant host state.

**Pros:** safe integration.  
**Cons:** many `glGet*` calls may be costly and can force driver synchronization.

### Option B — documented state contract

SparkGlass documents exactly what it modifies, and the host adapter restores what it needs.

**Pros:** efficient and explicit.  
**Cons:** stronger host integration contract.

### Option C — safe + fast modes

Possible, but only if real integration cases justify the complexity.

## Current leaning

Prefer a **documented, minimal state contract**, with correctness tests on GTK and WinUI, rather than blindly querying/restoring the entire GL state machine every frame.

Do not freeze this before benchmarking.

---

# 38. DPI AND COORDINATES

Host UI frameworks typically use logical coordinates while GPU render targets use physical pixels.

Example:

```text
logical UI:        800 × 600
scale factor:      2.0
physical target:  1600 × 1200
```

## PROVISIONAL

The host should provide:

- logical viewport dimensions or a clearly defined coordinate system;
- scale factor;
- target pixel dimensions where necessary.

SparkGlass converts consistently for GPU rendering.

Do not couple the core to:

- GTK scaling APIs;
- WinUI scaling APIs.

---

# 39. NATIVE UI OVERLAYS

Native UI controls should remain native.

Desired composition:

```text
Native toolkit composition
│
├── backdrop/content
├── SparkGlass-rendered surface
├── native text
├── native buttons
└── native interactive controls
```

SparkGlass provides the material canvas/effect.

GTK/WinUI continues to own widget semantics and input.

---

# 40. INTERACTION / MORPHING

Apple's Liquid Glass includes interaction-responsive behavior and shape morphing.

These are important for long-term fidelity but should not block the first architectural decoupling from Macroquad.

## FUTURE / QUALITY ROADMAP

Potential semantic inputs:

- pointer/touch location;
- pressed state;
- velocity;
- acceleration;
- element transition progress;
- identity/morph relationship;
- container spacing;
- focus/hover.

Do not expose a low-level animation engine prematurely.

The host owns application animation timing; SparkGlass may consume interaction/material state and render the correct optical response.

---

# 41. ADAPTIVE MATERIAL ANALYSIS

Apple publicly describes the material as adaptive to underlying content.

A future high-fidelity path may derive GPU-side information such as:

- local luminance;
- local dominant color;
- contrast;
- dynamic range;
- neighboring light spill.

Possible GPU-only implementation strategies:

```text
mip reductions
small analysis textures
fragment reduction passes
compute reduction (future backend)
```

## FROZEN

Do not perform production visual analysis by reading pixels back to CPU every frame.

---

# 42. PERFORMANCE PHILOSOPHY

Performance matters because Liquid Glass must be interactive.

But optimize the correct thing.

Priority:

```text
preserve appearance
      ↓
measure GPU cost
      ↓
optimize render graph
      ↓
reduce redundant work
      ↓
select capabilities/backend paths
```

Good optimization examples:

- crop blur to needed sampling region;
- reuse blur between elements in one container;
- merge compatible passes;
- cache stable intermediates;
- batch geometry;
- avoid repeated shader state changes;
- avoid unnecessary full-screen passes;
- resolution-aware blur where visually equivalent;
- use texture pools;
- avoid allocations per frame.

Bad optimization:

> “Remove refraction because blur is cheaper.”

That defeats the project.

---

# 43. ZERO CPU READBACK — PRECISE RULE

## Production rendering

No normal per-frame GPU → CPU readback.

Avoid:

```text
glReadPixels
GPU texture download
CPU analysis of rendered frame
CPU mask reconstruction
```

## Testing/debugging exception

Readback **may be allowed in test tools** for:

- screenshot capture;
- golden-image regression tests;
- pixel/perceptual diffing;
- diagnostics.

That exception must never leak into the production rendering path.

---

# 44. VISUAL REGRESSION TESTING — CRITICAL

Because appearance is priority #1, SparkGlass should eventually have a visual validation suite.

Test dimensions should include:

### Geometry

- capsule;
- rounded rectangle;
- superellipse;
- asymmetric dimensions;
- small controls;
- large panels.

### Backdrops

- dark;
- bright;
- high-frequency text/checker patterns;
- saturated artwork;
- gradients;
- video-like content.

### Optical behavior

- blur;
- refraction;
- edge distortion;
- chromatic dispersion;
- tint;
- specular edge;
- shadow;
- transparency.

### Scene interaction

- one glass element;
- multiple nearby elements;
- overlapping sampling regions;
- container merge distances;
- moving elements;
- DPI changes.

### Backend/platform

- desktop GL;
- ANGLE D3D11;
- ANGLE Vulkan if available;
- native Vulkan in the future.

## Golden-rule test

A new backend is not complete merely because it runs.

It must produce **visually equivalent material behavior** within an agreed tolerance.

---

# 45. LOCAL DEVELOPMENT SANDBOX

The production library is windowless, but developers need a runnable test environment.

## PROVISIONAL

```text
winit
  ↓
glutin
  ↓
GL context
  ↓
SparkGlass core
```

Possible command:

```bash
cargo run --example sandbox
```

The sandbox is allowed to own:

- a development window;
- a test event loop;
- test texture loading;
- parameter controls;
- FPS/profiling overlays;
- visual-regression capture.

The **production core** is not.

---

# 46. REPOSITORY STRUCTURE — NEAR-TERM

Do not split into many crates until necessary.

A reasonable starting organization:

```text
SparkGlass/
│
├── Cargo.toml
│
├── src/
│   ├── lib.rs
│   │
│   ├── scene/
│   │   ├── mod.rs
│   │   ├── container.rs
│   │   └── element.rs
│   │
│   ├── material/
│   │   ├── mod.rs
│   │   └── glass.rs
│   │
│   ├── renderer/
│   │   ├── mod.rs
│   │   ├── pipeline.rs
│   │   ├── render_graph.rs
│   │   ├── framebuffer.rs
│   │   ├── shader.rs
│   │   └── state.rs
│   │
│   ├── resource/
│   │   ├── mod.rs
│   │   └── texture.rs
│   │
│   ├── backend/
│   │   └── gl/
│   │       ├── mod.rs
│   │       ├── context.rs
│   │       ├── texture.rs
│   │       └── target.rs
│   │
│   └── ffi/
│       ├── mod.rs
│       ├── context.rs
│       ├── frame.rs
│       ├── texture.rs
│       └── error.rs
│
├── shaders/
│   ├── glass.frag
│   ├── glass_mask.frag
│   └── blur.frag
│
└── examples/
    └── sandbox.rs
```

This is organizational guidance, not a frozen tree.

---

# 47. POSSIBLE FUTURE WORKSPACE

Only after complexity justifies it:

```text
crates/
├── sparkglass-core/
├── sparkglass-gl/
├── sparkglass-ffi/
├── sparkglass-vulkan/      # future
└── platform-adapters/      # only if useful
```

Do not create this split merely for architectural aesthetics.

---

# 48. WINDOWS ADAPTER RESPONSIBILITY

A Windows/WinUI adapter may own:

- `SwapChainPanel` integration;
- EGL display/config/context creation;
- ANGLE setup;
- current-context management;
- DXGI/swapchain details;
- texture import interoperability;
- resize handling;
- presentation;
- render scheduling.

The SparkGlass core should see:

```text
current GLES context
valid render target
valid imported texture handles
frame/scene description
```

and nothing WinUI-specific.

---

# 49. LINUX ADAPTER RESPONSIBILITY

A GTK adapter may own:

- `GtkGLArea` widget;
- `realize` / `unrealize`;
- context currentness;
- render callback;
- GTK size/scale conversion;
- frame scheduling;
- extracting/current target information when required.

SparkGlass core should not include GTK types.

---

# 50. WHY GTK MAKES GL A STRONG FIRST BACKEND

GTK's GL integration is public and explicit.

`GtkGLArea` provides:

- context ownership;
- render callback;
- current framebuffer;
- transparent framebuffer initialization;
- scene-graph integration.

By contrast, GTK's previously exposed Vulkan context is deprecated and GTK states that it does not expose Vulkan internals.

Therefore native Vulkan may eventually require a different integration strategy.

This is a strong practical reason to deliver GL/GLES first.

---

# 51. WHAT MATTERS **RIGHT NOW**

## Tier 1 — absolutely important

1. Preserve visual fidelity.
2. Preserve the existing PoC as a visual reference.
3. Remove Macroquad from the production rendering dependency.
4. Host owns graphics context/window/UI.
5. Core renderer operates in host-current GL/GLES context.
6. Establish scene/container/element semantics.
7. Establish backdrop ownership and texture lifetime.
8. Keep the entire normal material pipeline GPU-resident.
9. Define context-loss lifecycle.
10. Define a stable C ABI boundary.
11. Keep GL implementation types out of the semantic model.
12. Validate the new GL renderer against the PoC visually.

---

# 52. WHAT MATTERS **SOON, BUT NOT FIRST**

- exact render-target contract;
- exact state-preservation contract;
- alpha semantics;
- color-space rules;
- capability querying;
- optimized shared blur regions;
- adaptive luminance;
- texture pooling;
- container SDF merging;
- morphing semantics;
- platform adapters.

---

# 53. WHAT DOES **NOT** MATTER YET

Do not derail the current work with:

- full native Vulkan backend;
- Metal backend;
- Direct3D backend;
- WebGPU backend;
- generalized cross-API render framework;
- a plugin system;
- network serialization;
- JSON scene protocol for every frame;
- UI framework ownership;
- animation framework replacement;
- full material editor;
- premature ECS;
- premature multithreaded renderer;
- premature shader-language transpiler.

Those can be revisited only when a real requirement appears.

---

# 54. IMPLEMENTATION PHASES

## Phase 0 — preserve reference

- keep Macroquad PoC runnable;
- capture reference scenes;
- record shader parameters;
- document expected appearance.

### Exit condition

We can identify visual regressions during migration.

---

## Phase 1 — freeze semantics

Define relationships between:

```text
Context
Scene / Frame
Backdrop
TextureHandle
Material
GlassContainer
GlassElement
RenderTarget
```

### Exit condition

Ownership and lifetime are understandable without discussing shader passes.

---

## Phase 2 — freeze C ABI v0 design

Define:

- opaque handles;
- ABI versioning;
- frame structs;
- pointer/count rules;
- ownership;
- errors;
- thread requirements;
- context lifecycle;
- texture import lifecycle.

### Exit condition

A C/C++ host could theoretically call SparkGlass without any Rust-specific knowledge.

---

## Phase 3 — decouple GL renderer

Replace Macroquad rendering calls with `glow` while preserving shaders/material behavior.

### Exit condition

Same visual scenes render without Macroquad inside the production core.

---

## Phase 4 — sandbox

Build `winit + glutin` example executable.

Test:

- resizing;
- DPI;
- multiple elements;
- multiple containers;
- texture input;
- transparent output;
- visual regression;
- GPU timings.

### Exit condition

SparkGlass is demonstrably windowless while still easy to develop locally.

---

## Phase 5 — Linux integration

Integrate with GTK 4 + `GtkGLArea`.

### Exit condition

Native GTK controls can compose correctly over/around SparkGlass with correct alpha and no CPU readback.

---

## Phase 6 — Windows integration

Integrate WinUI 3 + `SwapChainPanel` + ANGLE.

Test ANGLE backing paths where practical:

```text
D3D11
Vulkan
```

### Exit condition

Same semantic SparkGlass API works on Windows.

---

## Phase 7 — GoosicReborn integration

Use GoosicReborn as the first real-world architecture validation.

Question:

> Can GoosicReborn describe the same glass scene independent of whether it is running on GTK/Linux or WinUI/Windows?

If yes, the architecture boundary is working.

---

## Phase 8 — advanced fidelity/performance

Potential features:

- adaptive luminance;
- shared sampling regions;
- advanced container merging;
- interaction deformation;
- high-quality dispersion;
- ambient color spill;
- motion response;
- further GPU optimizations.

---

## Phase 9 — evaluate native Vulkan

Only after the GL/GLES version is stable and there is a concrete reason.

Evaluate:

- performance bottlenecks;
- interop benefits;
- Linux toolkit integration;
- Windows native integration;
- compute value;
- maintenance burden;
- visual parity.

Do **not** assume Vulkan wins automatically.

---

# 55. CRITICAL QUESTIONS STILL OPEN

## A. Exact GLES baseline

Is every visual feature required to work on strict GLES 3.0, or may quality tiers require optional GLES 3.1/extensions?

## B. Render target

Current FBO, explicit FBO, SparkGlass-owned output, or multiple modes?

> See `SparkGlass_IMPLEMENTATION_STATUS.md` → "The render-target bug" for a
> real bug this ambiguity caused (a same-process pixel readback can look
> correct while the host's actual framebuffer never gets written) and how
> option A is implemented today.

## C. GL state contract

Full preservation, documented subset, or optional modes?

> See `SparkGlass_IMPLEMENTATION_STATUS.md` → "GL state isolation
> experiment" for partial empirical evidence from a real GTK4 host: native
> widgets composited correctly with zero state save/restore on our side.
> One data point, not a closed decision.

## D. Color space

Exact linear/sRGB/HDR behavior?

## E. Container semantics

What does container `spacing` mean in our API? Is it only future morphing, or also sampling/optimization?

## F. Backdrops

One scene backdrop or multiple named backdrops?

## G. External texture synchronization

How does the host guarantee the imported texture is ready for sampling?

## H. Material properties

Which values are stable semantic properties vs implementation-specific shader tuning?

## I. Context recreation

Can one `SparkGlassContext` survive a GL context recreation by reinitializing GPU state, or must it be rebuilt?

## J. Alpha/compositor contract

Exact premultiplied behavior on WinUI and GTK?

---

# 56. CURRENT DECISION TABLE

| Area | Status | Direction |
|---|---:|---|
| Visual fidelity | **FROZEN** | Highest priority |
| Macroquad | **FROZEN** | PoC/reference only |
| Window ownership | **FROZEN** | Host |
| UI ownership | **FROZEN** | Host |
| Graphics-context ownership | **FROZEN** | Host |
| Presentation/swapchain | **FROZEN** | Host/platform adapter |
| Production renderer v1 | **PROVISIONAL** | GL/GLES |
| GL wrapper | **PROVISIONAL** | `glow` |
| Minimum API | **PROVISIONAL** | GLES 3.0-compatible baseline |
| Windows integration | **PROVISIONAL** | WinUI 3 + ANGLE |
| Linux integration | **PROVISIONAL** | GTK 4 + GtkGLArea |
| C ABI | **FROZEN principle** | `extern "C"`, ABI-safe |
| Rust implementation handle | **FROZEN principle** | Opaque |
| Hot-path scene data | **PROVISIONAL** | C structs/arrays |
| JSON render submission | **REJECTED** | Not for hot path |
| Frame model | **PROVISIONAL** | Batched scene description |
| Scene organization | **PROVISIONAL** | Containers + elements |
| SDF | **PROVISIONAL** | Central technique |
| External textures | **FROZEN ownership** | Host-owned |
| CPU readback | **FROZEN** | Never in normal rendering |
| Internal render thread | **FROZEN** | No, by default |
| GL thread | **FROZEN** | Host thread with current context |
| Render target | **OPEN** | Needs platform experiments |
| GL state restoration | **OPEN** | Prefer documented minimal contract, benchmark |
| Premultiplied alpha | **PROVISIONAL** | Preferred |
| Vulkan | **FUTURE** | Backend #2 candidate, not first implementation |
| Giant generic backend abstraction | **REJECTED now** | Wait for backend #2 |
| Native Apple private APIs | **REJECTED** | Architectural study only |

---

# 57. AI AGENT OPERATING INSTRUCTIONS

Any AI modifying or advising on this project must follow this hierarchy.

## Rule 1 — preserve the visual target

Before changing the renderer, ask:

> Will this preserve or improve the existing glass appearance?

If unknown, keep the existing path and build a comparison first.

## Rule 2 — do not confuse architecture cleanup with visual redesign

The Macroquad removal task is initially about **decoupling infrastructure**, not changing the material.

First reproduce the current visual pipeline under the new backend.

Then optimize/refine.

## Rule 3 — do not invent requirements

If this document marks a decision as OPEN, do not silently choose one and redesign the codebase around it.

Document the trade-off or run the necessary platform experiment.

## Rule 4 — keep platform APIs outside the core

Do not put these inside core scene/material modules:

```text
GtkWidget
GtkGLArea
SwapChainPanel
IDXGISwapChain
EGLDisplay
winit::Window
glutin context objects
```

## Rule 5 — do not leak backend resources into semantic structs

Avoid:

```text
GlassElement.texture: glow::NativeTexture
```

Use library handles/resource references.

## Rule 6 — no unnecessary CPU path

Do not solve GPU problems by adding per-frame screenshots/readback unless explicitly building test/debug tooling.

## Rule 7 — preserve shader pipeline opacity

Consumer APIs should never depend on shader filenames or ping-pong pass count.

## Rule 8 — benchmark before deleting correctness safeguards

Especially:

- GL state restoration;
- framebuffer sizing;
- texture import synchronization;
- alpha handling.

## Rule 9 — Vulkan is not the current migration target

Do not rewrite the project into Vulkan merely because Vulkan is more modern.

The current goal is to extract the material renderer from Macroquad cleanly.

## Rule 10 — explain non-obvious graphics decisions

The project owner should not be expected to already know every graphics API rule.

When proposing a decision, explain:

- what it means;
- why it matters visually;
- why it matters architecturally;
- performance implications;
- alternatives.

---

# 58. THINGS AN AI SHOULD NEVER DO WITHOUT EXPLICIT JUSTIFICATION

- Replace the current shaders with a simple Gaussian blur.
- Flatten the material to translucent rectangles.
- Remove refraction because it is expensive.
- Move rendering to CPU.
- Use `glReadPixels()` in production frame rendering.
- Make one OS implementation visually different simply because its API is easier.
- Create one SparkGlass context per button by default.
- Couple `GlassElement` to WinUI or GTK widgets.
- Create hidden windows/contexts inside the core library.
- Make SparkGlass own the host event loop.
- Add an internal render thread without a proven requirement.
- Require hosts to know internal pass ordering.
- Freeze ABI structs around today's exact GLSL uniforms.
- Assume raw GL texture IDs are portable across backends.
- Assume Vulkan automatically improves visual quality or performance.
- Treat reverse-engineered Apple private class names as stable API.
- Delete the PoC before a visual comparison path exists.

---

# 59. SUCCESS CRITERIA

SparkGlass is successful when all of the following are true:

### Visual

- the material convincingly matches the target Liquid Glass appearance;
- blur alone is not mistaken for glass;
- edge/lensing behavior remains present;
- geometry remains fluid and high quality;
- grouped elements can eventually behave coherently;
- native UI composition does not create halos/artifacts.

### Architectural

- core owns no window;
- core owns no UI toolkit;
- core owns no platform event loop;
- core can render inside a host-owned graphics context;
- same semantic API is usable on Windows and Linux;
- C ABI exposes no Rust-specific types;
- internal renderer can evolve without changing the material scene model.

### Performance

- no normal GPU readback;
- no unnecessary CPU texture bounce;
- frame work can be batched;
- internal buffers are reused;
- host UI remains responsive;
- GPU passes are optimized only after visual parity.

---

# 60. MENTAL MODEL IN ONE DIAGRAM

```text
                    ┌────────────────────────────┐
                    │      HOST APPLICATION      │
                    │     layout + native UI     │
                    └─────────────┬──────────────┘
                                  │
                    scene + GPU backdrop + target
                                  │
                ┌─────────────────┴─────────────────┐
                │                                   │
        Windows adapter                       Linux adapter
      WinUI + SwapChainPanel                   GTK + GLArea
          EGL / ANGLE                              GL
                │                                   │
                └─────────────────┬─────────────────┘
                                  │
                         current GL/GLES context
                                  │
                     ┌────────────▼────────────┐
                     │     SPARKGLASS C ABI    │
                     └────────────┬────────────┘
                                  │
                     ┌────────────▼────────────┐
                     │     Semantic Scene      │
                     │ backdrop                │
                     │ containers[]            │
                     │   elements[]            │
                     │ materials               │
                     └────────────┬────────────┘
                                  │
                     ┌────────────▼────────────┐
                     │    Material Renderer    │
                     │ geometry / SDF          │
                     │ blur                    │
                     │ refraction              │
                     │ adaptivity              │
                     │ lighting / shadow       │
                     └────────────┬────────────┘
                                  │
                           backend implementation
                                  │
                     ┌────────────▼────────────┐
                     │   GL/GLES via `glow`    │
                     │        BACKEND #1       │
                     └────────────┬────────────┘
                                  │
                                  ▼
                                 GPU

                     FUTURE, ONLY IF JUSTIFIED:
                                  │
                     ┌────────────▼────────────┐
                     │   Native Vulkan backend │
                     └─────────────────────────┘
```

---

# 61. THE ONE-SENTENCE ARCHITECTURE

> **SparkGlass is a windowless, GPU-resident, scene-aware visual material engine in Rust that receives host-owned GPU backdrop resources and glass geometry through a stable C ABI, renders Apple-style Liquid Glass using an internally controlled multi-pass/SDF pipeline inside a host-owned graphics context, and keeps platform/UI/context ownership outside the core.**

---

# 62. THE ONE-SENTENCE PRIORITY RULE

> **Never sacrifice the defining visual behavior of the material merely to make the architecture simpler.**

---

# 63. REFERENCES / EVIDENCE

## Project context

- Original SparkGlass Architectural Design Brief — source document used to produce this master specification.
- GoosicReborn: https://github.com/AnalogicGoose/GoosicReborn/tree/development

## Apple — public documentation

- Apple — Applying Liquid Glass to custom views  
  https://developer.apple.com/documentation/SwiftUI/Applying-Liquid-Glass-to-custom-views

- Apple — GlassEffectContainer  
  https://developer.apple.com/documentation/swiftui/glasseffectcontainer

- Apple — Meet Liquid Glass (WWDC25)  
  https://developer.apple.com/videos/play/wwdc2025/219/

- Apple — Build an AppKit app with the new design (WWDC25)  
  https://developer.apple.com/videos/play/wwdc2025/310/

## Video discussed

- Gui Rambo — Inside Liquid Glass: How iOS 26 Synthesizes Refractive UI  
  https://www.youtube.com/watch?v=oj20mb8c0yI

- Swift Connection 2025 video index  
  https://async.techconnection.io/frenchkit/

## Apple private-implementation reverse engineering — informational only

- iOS 26 Liquid Glass reverse engineering / SDF + backdrop investigation  
  https://lrdcq.com/me/read.php/165.htm

- ShatteredGlass — Core Animation / SDF deconstruction experiment  
  https://github.com/AlexStrNik/ShatteredGlass

> [!WARNING]
> These reverse-engineering references involve private implementation observations. They are useful for learning architectural ideas, not as stable APIs or specifications.

## GTK

- GtkGLArea  
  https://docs.gtk.org/gtk4/class.GLArea.html

- GdkVulkanContext — deprecated; GTK notes it does not expose Vulkan internals  
  https://docs.gtk.org/gdk4/class.VulkanContext.html

## ANGLE

- ANGLE repository / supported translation backends  
  https://github.com/google/angle

- ANGLE development / backend selection  
  https://github.com/google/angle/blob/main/doc/DevSetup.md

- ANGLE Vulkan backend notes  
  https://github.com/google/angle/blob/main/src/libANGLE/renderer/vulkan/README.md

---

# 64. FINAL NOTE TO FUTURE CONTRIBUTORS AND AI AGENTS

The objective is **not** to build the most academically elegant renderer.

The objective is to build a **clean, native, fast, reusable engine that visually behaves like Liquid Glass** and integrates naturally with real desktop UI frameworks.

When uncertain, return to this order:

```text
Does it LOOK right?
        ↓
Does it composite correctly?
        ↓
Is it GPU-resident?
        ↓
Is ownership/lifetime correct?
        ↓
Is it fast enough?
        ↓
Is the public API clean?
```

The current shader PoC is evidence of the target appearance.

The architecture exists to preserve that appearance while making it native, embeddable, maintainable, and cross-platform.

**Visual fidelity is the product. Everything else is infrastructure.**
