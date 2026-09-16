# SparkGlass — Master Development Roadmap

> **Visual fidelity is the product. Everything else is infrastructure.**

SparkGlass is a cross-platform GPU material engine written in Rust, focused on reproducing the visual behavior of Apple's modern Liquid Glass material for native desktop applications.

This roadmap exists to keep development focused and to prevent future contributors or AI agents from optimizing the wrong things, rewriting working systems prematurely, or treating SparkGlass as a generic blur library.

---

# 0. North Star

SparkGlass succeeds only if it looks excellent.

The project must prioritize, in this order:

1. **Visual fidelity to real Apple Liquid Glass behavior**
2. **Correct material architecture**
3. **GPU correctness and stability**
4. **Cross-platform native integration**
5. **Performance optimization**
6. **Backend expansion**

A cleaner architecture that visibly reduces quality is not an improvement.

A faster renderer that loses the optical character of the material is not an improvement.

A feature is not complete because it compiles.

---

# 1. Visual Authority Order

When references disagree, use this priority order:

```text
1. Real Apple Liquid Glass behavior on shipping Apple platforms
2. Apple public documentation and WWDC demonstrations
3. Reproducible Apple screenshots / recordings / device observations
4. Reverse-engineered Apple compositor/layer behavior
5. Figma Liquid Glass reference components
6. Existing SparkGlass PoC behavior
```

## Important

The Figma component is a **calibration reference**, not absolute truth.

Values such as:

```text
Refraction = 70
Opacity = 25
```

must **not** be copied directly into SparkGlass uniforms unless their semantics and units are proven equivalent.

The existing SparkGlass PoC remains valuable because it preserves working visual behavior, but it is not the final authority if real Apple behavior contradicts it.

---

# 2. Core Product Definition

SparkGlass is:

> A native GPU material engine that renders Apple-inspired Liquid Glass surfaces inside host-owned desktop UI environments.

SparkGlass is **not**:

- a UI toolkit
- a layout engine
- a game engine
- a windowing framework
- a GTK replacement
- a WinUI replacement
- an ANGLE wrapper
- a generic post-processing engine
- a basic blur library

The host owns:

- application window
- UI toolkit
- layout
- event loop
- native controls
- graphics context
- presentation

SparkGlass owns:

- glass material rendering
- SDF geometry processing
- backdrop processing
- blur/scattering
- refraction/lensing
- material highlights
- surface response
- GPU resource management internal to SparkGlass
- glass-specific render scheduling

---

# 3. Target Architecture

```text
                      Host Application
                             │
                 ┌───────────┴───────────┐
                 │                       │
              Windows                  Linux
                 │                       │
              WinUI 3                  GTK 4
                 │                       │
          SwapChainPanel              GtkGLArea
                 │                       │
               ANGLE                 OpenGL/GLES
                 │                       │
                 └───────────┬───────────┘
                             │
                     Host-owned GL context
                             │
                             ▼
                       SparkGlass C ABI
                             │
                             ▼
                    Semantic Glass Scene
                             │
                             ▼
                       Render Planning
                             │
                             ▼
                      GL/GLES Renderer
                             │
                            glow
                             │
                             ▼
                             GPU
```

Long-term:

```text
                    SparkGlass Core
                         │
               ┌─────────┴─────────┐
               ▼                   ▼
          GL/GLES backend      Vulkan backend
               │                   │
             glow                  ash
```

The Vulkan branch is a **future possibility**, not the current implementation target.

---

# 4. Current Proven Foundation

The current development state has already validated important architectural ideas.

## Proven / substantially validated

- Rust implementation
- C-compatible ABI direction
- `glow`-based rendering direction
- standalone development sandbox
- GTK integration path
- native GTK controls over SparkGlass rendering
- host-provided texture path
- real C consumer path
- backend regression comparison
- multi-pass GPU rendering
- ping-pong framebuffer workflow
- SDF/procedural glass geometry
- shader-based blur/refraction/compositing

## Important discovered bug class

Render-target correctness must be verified outside SparkGlass itself.

A previous issue showed that an internal framebuffer could remain bound while presentation assumed the host framebuffer was active.

Therefore:

> Internal pixel tests are insufficient to prove host presentation correctness.

All platform integrations need both:

```text
internal render validation
+
external/on-screen presentation validation
```

---

# 5. Roadmap Overview

```text
PHASE 0   Preserve the visual reference
PHASE 1   Semantic scene model
PHASE 2   Stable C ABI
PHASE 3   Production GL/GLES renderer
PHASE 4   Sandbox + visual regression framework
PHASE 5   Linux / GTK integration
PHASE 6   Windows / WinUI + ANGLE integration
PHASE 7   Real application validation
PHASE 8   Apple Material Fidelity
PHASE 9   Container interaction + motion
PHASE 10  Performance architecture
PHASE 11  Backend expansion / Vulkan investigation
```

The project is now entering the most important visual phase:

# **PHASE 8 — Apple Material Fidelity**

---

# PHASE 0 — Preserve the Visual Reference

## Goal

Ensure the current PoC can never be accidentally lost while the new architecture evolves.

## Required work

- preserve the Macroquad PoC
- tag a known-good visual reference commit
- preserve all current shaders
- preserve representative screenshots / recordings
- preserve parameter values used by the reference implementation

Suggested tag:

```text
v0.1-poc-reference
```

## Acceptance criteria

- visual reference launches independently
- existing shader behavior is reproducible
- visual reference assets are versioned
- future renderer can be compared against it

## Do not

- delete Macroquad merely because the new renderer compiles
- rewrite working shaders before reference capture exists

---

# PHASE 1 — Semantic Scene Model

## Goal

Define what SparkGlass renders without exposing how the GPU executes it.

## Core model

```text
GlassScene
│
├── Backdrop
│
├── GlassContainer[]
│   └── GlassElement[]
│
└── RenderTarget
```

## GlassElement should describe

- position
- dimensions
- shape
- corner geometry
- transform
- material reference
- opacity
- interaction state

## GlassContainer should describe

- grouping relationship
- shared backdrop/sampling semantics
- container transform
- container-level material behavior if required

## Acceptance criteria

The semantic model contains **no requirement** to understand:

- ping-pong FBOs
- shader program IDs
- GL texture units
- blur passes
- ANGLE
- GTK
- WinUI

---

# PHASE 2 — Stable C ABI

## Goal

Expose SparkGlass as a standalone native library without leaking Rust implementation details.

## Core rules

Use:

```c
typedef struct SparkGlassContext SparkGlassContext;
```

Do not expose:

- Rust references
- Rust generics
- Rust slices
- `Vec<T>`
- Rust strings
- Rust enum layout assumptions
- `dyn Trait`
- `glow` native types in semantic structures

## FFI model

Prefer:

- opaque handles
- POD C structs
- pointer/count arrays
- explicit ownership
- explicit versioning
- explicit error codes

## Important resource abstraction

Do not spread raw `GLuint` values throughout semantic APIs.

Preferred direction:

```text
Native GL Texture
       │
       ▼
SparkGlass import function
       │
       ▼
SGTextureHandle
       │
       ▼
GlassScene
```

This enables future alternate imports without redesigning the material model.

---

# PHASE 3 — Production GL/GLES Renderer

## Goal

Remove Macroquad from the production renderer while preserving visual behavior.

## Backend

```text
SparkGlass
    ↓
  glow
    ↓
OpenGL / OpenGL ES 3.x
```

## Baseline

Target approximately:

```text
OpenGL ES 3.0+
```

Use newer features only where optional and beneficial.

## Constraints

- host owns context
- host makes context current
- SparkGlass does not create the production window
- SparkGlass does not create the production event loop
- SparkGlass does not own presentation

## Acceptance criteria

- shaders match or exceed PoC visuals
- no normal CPU readback
- internal resources persist across frames
- resize behavior is correct
- context destruction/recreation behavior is defined

---

# PHASE 4 — Sandbox + Visual Regression Framework

## Goal

Make visual development independent from GTK/WinUI.

## Sandbox

```text
winit
  +
glutin
  +
SparkGlass
```

## Required test scenes

Build fixed scenes for:

- bright backdrop
- dark backdrop
- colorful album artwork
- high-frequency text/background
- gradients
- controls over glass
- small pill glass
- large panel glass
- overlapping/grouped glass
- animated glass geometry

## Visual regression system

Store:

```text
reference image
candidate image
difference image
material parameters
GPU/backend metadata
```

## Required validation types

1. Internal renderer capture
2. External host/window capture

Never accept only an internal offscreen result as proof that integration is correct.

---

# PHASE 5 — Linux / GTK Integration

## Goal

Validate SparkGlass as a native material surface inside GTK 4.

## Target path

```text
GTK 4
 ↓
GtkGLArea
 ↓
host GL context
 ↓
SparkGlass
```

## Required tests

- resize
- HiDPI
- native GTK widgets above glass
- texture import
- context recreation
- transparent composition
- multiple SparkGlass elements
- dirty GL state behavior

## GL state contract

Current evidence suggests a minimal state contract may be viable, but this is **not yet frozen**.

Possible final models:

```text
A. SparkGlass restores all relevant state
B. SparkGlass restores a documented subset
C. Host adapter restores the state it requires
D. Safe mode + fast mode
```

The final decision must be based on measurements, not aesthetics.

---

# PHASE 6 — Windows / WinUI 3 + ANGLE Integration

## Goal

Validate native Windows embedding without coupling the SparkGlass material core to Windows APIs.

## Target path

```text
WinUI 3
   ↓
SwapChainPanel
   ↓
ANGLE / EGL
   ↓
OpenGL ES
   ↓
SparkGlass
```

SparkGlass should not need to know whether ANGLE internally uses:

- D3D11
- Vulkan
- another supported backend

## Required tests

- DXGI/SwapChainPanel presentation
- alpha composition
- resize
- DPI scaling
- host/native controls above glass
- external texture path
- context/surface recreation
- render-target restoration
- ANGLE backend differences

## Important

WinUI/ANGLE state behavior must be measured independently from GTK.

GTK success does not prove ANGLE success.

---

# PHASE 7 — Real Application Validation

## Goal

Use a real application to prove SparkGlass is not only a demo renderer.

Primary validation target:

```text
GoosicReborn
```

## Success condition

The same material description should work on:

```text
Windows / WinUI / ANGLE
Linux / GTK / GLArea
```

Only the platform integration layer should differ.

## Required real-world scenarios

- album artwork backdrop
- music controls
- sidebars
- navigation surfaces
- floating control pills
- now-playing panels
- animated state changes

---

# PHASE 8 — Apple Material Fidelity

> **This is the highest-priority visual phase.**

The purpose of Phase 8 is not to add random effects.

It is to move SparkGlass from “convincing refractive glass” toward a material system that behaves much more like Apple's Liquid Glass.

---

## PHASE 8.1 — Material Style System

### Goal

Stop treating every glass surface as the same material profile.

### Needed style families

Candidate families:

```text
Clear
Regular
Prominent
Control
Navigation
Small/Pill
Large/Panel
Light appearance
Dark appearance
```

Names may change. What matters is that material behavior is style-specific.

### Tune per style

- frost/scattering
- tint
- transmission
- depth
- refraction
- highlight strength
- edge response
- chromatic behavior
- shadow

### Acceptance criteria

A small control pill and a large navigation bar should not look like the same shader merely scaled to different dimensions.

---

## PHASE 8.2 — Optical Calibration

### Goal

Calibrate the physical-looking body of the material.

### Focus

- blur / scattering
- lens depth
- inner refraction region
- displacement magnitude
- material thickness
- tint
- transparency
- chromatic dispersion where appropriate

### Critical warning

Do not directly import Figma numeric values unless their mapping is proven.

Example:

```text
Figma Refraction = 70
```

must not automatically become:

```text
u_refraction = 70
```

because the units and response curves are unrelated until calibrated.

### Calibration method

Use:

```text
reference scene
+
parameter sweep
+
side-by-side comparison
+
human visual evaluation
```

Do not rely on one developer's memory.

---

## PHASE 8.3 — SDF Geometry Fidelity

### Goal

Treat glass geometry as a real material input, not merely a mask.

### Desired SDF data

Potentially:

```text
R → signed/unsigned distance
G → nearest-boundary normal/direction X
B → nearest-boundary normal/direction Y
```

The exact encoding can differ, but SparkGlass needs enough geometric information to support high-quality lensing and edge response.

### Required behavior

- consistent superellipse/rounded geometry
- stable normals
- smooth corners
- no discontinuity during resizing
- stable refraction near boundaries

---

## PHASE 8.4 — Refraction & Lensing Fidelity

### Goal

Make the glass behave optically rather than as a blurred card.

### Model

```text
SDF distance
+
SDF normal
+
material depth
+
refraction amount
        ↓
sampling displacement
        ↓
refracted backdrop
```

### Desired behavior

- stronger displacement near glass boundaries
- calmer center regions where appropriate
- style-specific depth
- no obvious texture tearing
- no unstable edge sampling
- no hard discontinuity between blur and refraction zones

---

## PHASE 8.5 — Adaptive Material Response

### Goal

Make glass react to the backdrop instead of using fixed global values.

### Inputs

- local luminance
- local contrast
- backdrop color characteristics

### Outputs

Potentially adapt:

- tint
- exposure
- shadow strength
- highlight strength
- transmission
- local contrast response

### GPU rule

All analysis must remain GPU-resident during normal rendering.

No `glReadPixels()` feedback loop.

Possible future techniques:

- mip-chain analysis
- small reduction textures
- compute shader reduction

---

## PHASE 8.6 — Surface Response

### Goal

Reproduce the dimensional edge/light character of Apple Liquid Glass.

Treat the surface as more than a border.

### Target contributions

```text
Highlight Field A
Highlight Field B
Adaptive rim response
Specular response
Edge transmission behavior
```

### Important

The two directional highlight contributions should remain conceptually independent even if implementation later fuses them into one shader pass.

Do not reduce this to:

```text
1 px white border
```

---

## PHASE 8.7 — Shadow & Depth

### Goal

Make glass feel separated from the content behind it.

### Desired behavior

- style-dependent shadow
- backdrop-aware depth
- larger glass may feel deeper/heavier
- busy backgrounds may require stronger separation

### Important

Shadow visual z-order and shadow calculation dependency are separate concepts.

The shadow may be composited behind the glass while still depending on backdrop analysis.

---

## PHASE 8.8 — Clear Material Dimming

### Goal

Support clear/high-transmission styles without destroying foreground legibility.

### Model

```text
foreground content
      ▲
clear glass
      ▲
local dimming
      ▲
backdrop
```

This should be style-dependent, not globally applied.

---

## PHASE 8.9 — Composition Reference Model

SparkGlass should conceptually model the material as:

```text
BACK / SCENE

Original backdrop
Adaptive cast shadow
Optional clear-material dimming
Blur / scattering
Refracted / lensed backdrop
Adaptive tint / transmission
Highlight field A
Highlight field B
Adaptive rim / surface response
Interaction illumination
Native foreground content

FRONT / VIEWER
```

This is a semantic composition model.

It does **not** require one GPU pass per line.

Passes may be fused if fidelity is preserved.

---

# PHASE 9 — Container Interaction + Motion

## Goal

Move from independent glass objects to coherent multi-element material behavior.

---

## PHASE 9.1 — Explicit Glass Containers

Model:

```text
GlassContainer
 ├── GlassElement A
 ├── GlassElement B
 └── GlassElement C
```

Containers should communicate that these elements belong to one material region.

---

## PHASE 9.2 — Shared Sampling Regions

### Goal

Related elements should be able to share backdrop processing.

Benefits:

- more coherent optics
- fewer redundant blur passes
- correct neighboring glass behavior
- better transitions/morphing

---

## PHASE 9.3 — Shared / Merged SDF

### Goal

Allow grouped elements to contribute to a common geometry field.

Potential flow:

```text
Element A geometry ─┐
Element B geometry ─┼─→ merged shape field → SDF
Element C geometry ─┘
```

Do not require this optimization before style-level Phase 8 work is complete.

---

## PHASE 9.4 — Morphing

### Goal

Support smooth material transitions when related glass shapes approach, separate, resize, or transform.

Morphing must affect:

- geometry
- SDF
- highlights
- refraction
- interaction lighting

not merely alpha.

---

## PHASE 9.5 — Interaction Illumination

### Goal

Allow touch/pointer interaction to produce internal material illumination.

Conceptually:

```text
pointer/touch
    ↓
energy field
    ↓
local glow
    ↓
edge response
    ↓
optional propagation to related glass
```

---

# PHASE 10 — Performance Architecture

> Optimization begins after the material behavior is correct enough to measure meaningfully.

## 10.1 Shared Blur Reuse

Identify elements that can reuse the same filtered backdrop.

## 10.2 Backdrop Region Cropping

Avoid filtering the entire viewport when only a bounded sampling region is required.

## 10.3 Persistent Render Resources

Avoid recreating:

- textures
- FBOs
- shader programs
- buffers

per frame.

## 10.4 Reduced State Queries

Avoid expensive `glGet*` calls in the hot path where possible.

## 10.5 Material Batching

Group elements with compatible processing requirements.

## 10.6 GPU Timing

Measure:

- geometry/SDF cost
- blur cost
- refraction cost
- highlight cost
- composite cost

## Acceptance criteria

Optimization must be evaluated against both:

```text
performance improvement
AND
visual parity
```

---

# PHASE 11 — Vulkan / Future Backend Investigation

## Status

**NOT YET.**

Native Vulkan is strategically interesting, but it should not interrupt the current GL/GLES fidelity roadmap.

## Why Vulkan may matter later

Potential advantages:

- explicit synchronization
- explicit resource lifetimes
- compute workflows
- external memory/image interop
- better control of render scheduling
- backend-level performance opportunities

## Why not now

- GL/GLES maps cleanly to GtkGLArea
- ANGLE already gives Windows a GLES path
- ANGLE can potentially use Vulkan internally
- native Vulkan integration with GTK is less direct
- building Vulkan now would duplicate effort before material behavior is stable

## Required prerequisite

Do not implement a native Vulkan backend until:

- GL/GLES renderer is stable
- material semantics are stable
- C ABI is stable enough
- Apple Material Fidelity phase is mature
- performance data proves a reason to add Vulkan

---

# 12. Render Target Contract — Still Open

This remains an architectural decision requiring platform testing.

Possible models:

```text
A. Render into current framebuffer
B. Host passes target framebuffer
C. SparkGlass owns output texture
D. Support more than one mode
```

## Requirement

The semantic scene model should not be unnecessarily tied to the GL framebuffer representation.

A platform/render-specific target structure may be appropriate.

---

# 13. GL State Contract — Still Open

Possible strategies:

```text
FULL SAFE MODE
save/restore broad GL state

DOCUMENTED SUBSET
SparkGlass restores only explicitly documented state

HOST-RESTORES
SparkGlass leaves state unspecified and host adapter restores required state

DUAL MODE
safe + fast contracts
```

## Decision rule

Benchmark on:

- GTK / Mesa
- GTK / NVIDIA if possible
- ANGLE / D3D backend
- ANGLE / Vulkan backend if available

Do not freeze this based on one successful GTK test.

---

# 14. External Texture Contract

Host-provided GPU textures are central to SparkGlass.

## Ownership

The host owns imported external textures unless explicitly transferred.

SparkGlass may sample them.

SparkGlass must not destroy them.

## GL constraint

Imported native GL textures must be valid in the current GL context/share group.

## Future

Potential future imports:

```text
GL texture
EGL image
D3D shared resource
Vulkan image/external memory
```

These should map into SparkGlass resource handles rather than infecting the semantic material API.

---

# 15. Testing Matrix

Every major visual feature should be tested across:

## Backdrops

- black
- white
- grayscale
- colorful album art
- text-heavy UI
- gradients
- high-frequency detail
- video/animated texture

## Geometry

- small pills
- large panels
- circular controls
- extreme radii
- superellipses
- transformed elements
- overlapping elements

## Scale

- 1×
- 1.25×
- 1.5×
- 2×
- high-DPI fractional scaling where applicable

## Motion

- translation
- resize
- radius animation
- opacity transitions
- container morphing

## Platforms

- sandbox
- GTK
- WinUI/ANGLE

---

# 16. Visual Acceptance Criteria

Every material change should be reviewed for:

- edge quality
- refraction continuity
- blur quality
- tint accuracy
- highlight shape
- shadow depth
- backdrop readability
- foreground legibility
- temporal stability
- resize stability
- animation stability
- aliasing
- color banding
- halo artifacts
- black/transparent edge artifacts

A change that improves one category but visibly damages another requires review.

---

# 17. What Matters Most Right Now

## CURRENT PRIORITY

```text
1. Material style system
2. Optical calibration
3. SDF geometry fidelity
4. Refraction/lensing behavior
5. Apple-like highlight structure
6. Adaptive tint/luminance
7. Shadow/depth
8. Container semantics
```

## NEXT

```text
9. Shared sampling regions
10. Shared SDF behavior
11. Interaction illumination
12. Morphing
13. Performance optimization
```

## LATER

```text
14. Compute experiments
15. native Vulkan backend
16. advanced external-memory interop
```

---

# 18. What Does NOT Matter Yet

Do not spend major development time now on:

- a giant generic graphics-backend trait system
- native Vulkan implementation
- Metal backend
- WebGPU backend
- speculative mobile support
- plugin ecosystems
- shader scripting language
- runtime material graph editor
- generalized scene graph unrelated to glass
- premature micro-optimizations

These are distractions unless a real requirement appears.

---

# 19. AI / Contributor Guardrails

## Always

- preserve visual quality
- use real references
- distinguish documented Apple behavior from reverse-engineered behavior
- distinguish reverse-engineered behavior from SparkGlass inference
- preserve GPU-only normal rendering
- protect the host-owned context model
- keep platform-specific code outside the material core
- validate actual presented output
- benchmark before freezing performance contracts

## Never

- simplify SparkGlass into blur + opacity
- claim Figma parameter numbers map directly to shader uniforms without proof
- remove the PoC before parity exists
- use CPU screenshots in the normal render path
- introduce JSON into the per-frame hot path
- expose Rust implementation details over the C ABI
- expose `glow` throughout the semantic scene model
- rewrite the project around Vulkan before the GL/GLES material is mature
- claim internal framebuffer tests prove host presentation
- treat one platform's GL state behavior as universal

---

# 20. Definition of Done for SparkGlass 1.0

SparkGlass 1.0 should not mean “the library builds.”

It should mean:

- stable native C API
- production GL/GLES renderer
- Linux GTK integration validated
- Windows WinUI/ANGLE integration validated
- no CPU readback in normal rendering
- host GPU texture import supported
- scene/container/element model stable
- multiple material styles supported
- Apple-like refraction/lensing behavior
- strong surface highlights and edge response
- adaptive backdrop-aware material behavior
- stable HiDPI rendering
- stable resize/context lifecycle
- native UI overlays validated
- visual regression suite established
- real application integration proven
- documentation explains ownership, threading, resources, and lifecycle clearly

Most importantly:

> A developer should be able to place SparkGlass behind native controls and immediately recognize the material as a serious Liquid Glass implementation—not ordinary glassmorphism.

---

# 21. Final Rule

Whenever a roadmap decision becomes unclear, ask:

> **Does this make SparkGlass look and behave more like excellent Liquid Glass without damaging the architecture required to ship it?**

If yes, prioritize it.

If no, it can wait.

---

**SparkGlass**  
**Visual fidelity is the product. Everything else is infrastructure.**
