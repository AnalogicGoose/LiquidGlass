# SparkGlass — Implementation Status

> Companion to [`SparkGlass_MASTER_ARCHITECTURE.md`](SparkGlass_MASTER_ARCHITECTURE.md) (design rationale, changes rarely) and [`SparkGlass_ROADMAP.md`](SparkGlass_ROADMAP.md) (the authoritative phase plan from Phase 8 onward — it renumbers/expands what the master doc calls "Phase 8" into Phases 8–11 plus several supporting sections; where the two disagree on phase numbering, the roadmap wins, since it's newer). This document tracks what has actually been built and verified, and is expected to change often. Phase numbers below follow the roadmap.

## Summary

| Phase | What | Status |
|---|---|---|
| 0 | Preserve the Macroquad reference | Done — `cargo run` is still the visual baseline |
| 1 | Semantic scene model | Ongoing field-by-field audit — 2 rounds done, see below; not yet frozen on purpose |
| 2 | C ABI v0 | Done — `src/ffi/`, proven from a real C program, not just Rust |
| 3 | `glow` GL renderer | Done — `src/backend/gl/` |
| 4 | Standalone windowless-core sandbox + visual regression framework | Done — `examples/sandbox.rs`, `examples/visual_regression.rs` + `scripts/visual_regression.sh` |
| 5 | Linux (GTK4) integration | Done — `examples/gtk_glarea.rs`, `examples/gtk_glarea_overlay.rs` |
| 6 | Windows (WinUI 3 + ANGLE) integration | **Started, partially verified** — see below. No Windows dev machine here; using GitHub Actions windows-latest runners as the actual verification |
| 7 | GoosicReborn integration | Not started |
| 8 | Apple Material Fidelity | **Done** — 8.1–8.9 all addressed, see below (8.5/8.8's new knobs are shipped infrastructure, not yet product-tuned beyond "off") |
| 9 | Container interaction + motion | **Started** — 9.1 (Explicit Glass Containers) done at the scene-model level; 9.2–9.5 not started, see below |
| 10 | Performance architecture | Not started (roadmap says this waits until material behavior is correct enough to measure meaningfully) |
| 11 | Vulkan / future backend investigation | Not started (explicitly not the current target) |

Everything below is either implemented-and-verified or an explicit gap — nothing here is aspirational. If it doesn't have a "Status" note, it hasn't been built.

---

## Phase 1 — Semantic scene model (ongoing audit)

Not "frozen on purpose" yet — informally in decent shape, but never
deliberately reviewed field-by-field. The method used here: grep every
field's write-sites against its read-sites before trusting what a doc
comment claims it does. Two rounds so far:

- `GlassScene::new` was fabricating a hardcoded demo `GlassGroup` nothing
  ever read — fixed (now defaults to empty; see the "container/group
  semantics" note in `src/glass.rs`).
- `GlassMaterial::saturation`/`brightness`/`contrast`, `GlassOptics::
  surface_curvature`, and `GlassSurface::interaction` are part of the
  public struct but have **zero** corresponding shader uniform — confirmed
  by grep: every write-site sets a constant, no read-site exists in either
  renderer. Setting these from host code silently does nothing today. Not
  removed (they're reasonable future material properties, and
  `interaction` has a clear home in roadmap Phase 9.5), but now documented
  directly on the fields so nobody — human or AI — assumes they work.
  `GlassScene::reduced_transparency` was checked too and *is* genuinely
  wired (both renderers zero frost when it's set); `reduced_motion` isn't
  yet, for the honest reason that there's no motion/animation system at
  all for it to affect yet.

**Not done:** the rest of the roadmap's Phase 1 checklist (Backdrop,
TextureHandle beyond the FFI layer, RenderTarget) hasn't had the same
grep-audit treatment yet.

---

## Phase 2 — C ABI v0

`src/ffi/` (`context.rs`, `frame.rs`, `error.rs`, `texture.rs`) exposes:

```text
sg_create / sg_destroy / sg_resize /
sg_import_gl_texture / sg_release_texture / sg_set_backdrop /
sg_render_frame / sg_present / sg_last_error
```

Backdrop textures go through an explicit import step —
`sg_import_gl_texture(ctx, gl_texture_id, width, height) -> SGTextureHandle`
(an opaque `u64`, `0` always invalid) — rather than taking a raw `GLuint`
directly on every call. This follows §22 of the master doc, which explicitly
says the public/FFI surface must not depend directly on `GLuint`:
`GLuint → sg_import_gl_texture(...) → SGTextureHandle → scene references the
handle`. `sg_set_backdrop(ctx, handle)` and `sg_release_texture(ctx, handle)`
consume it from there. The host still owns the underlying GL texture in all
of this — `sg_release_texture` only forgets SparkGlass's reference, it never
calls `glDeleteTextures`.

All `extern "C"`, all panic-safe (`catch_unwind` at every boundary — no Rust
unwind ever crosses it), all operating only on the opaque `SparkGlassContext`
pointer and POD `SGFrame`/`SGGlassElement` structs.

Verified two ways:

- **`examples/ffi_smoke.rs`** drives the renderer through these functions
  only (no `GlGlassRenderer`, no `GlassScene`) and its captured output is
  byte-identical to `examples/sandbox.rs`'s direct-API output — the wrapper
  adds no behavioral divergence.
- **`c_smoke/main.c` + `include/spark_glass.h`** prove the exit condition
  literally: a plain C program, built with `gcc`, linking directly against
  the crate's `cdylib` output (`libspark_glass.so`) — no Rust anywhere in
  that build. It uploads a real GLES2 texture, imports it with
  `sg_import_gl_texture`, sets it as the backdrop, and confirms an unknown
  handle is rejected with `SG_ERROR_INVALID_TEXTURE`; then it creates a
  headless EGL pbuffer context and calls
  `sg_create`/`sg_render_frame`/`sg_present`, calls `sg_resize` and confirms
  a full render/present cycle still succeeds at the new size (this path was
  previously untested through the C ABI — Phase 3's own acceptance criteria
  explicitly call out "resize behavior is correct"), then deliberately
  submits an `SGFrame` with a wrong `struct_size` to confirm the ABI-version
  guard is actually enforced across a real FFI boundary; then it calls
  every context-taking function (`sg_resize`, `sg_import_gl_texture`,
  `sg_release_texture`, `sg_set_backdrop`, `sg_render_frame` — both a NULL
  context and a NULL frame pointer — `sg_present`, `sg_last_error`,
  `sg_destroy`) with a `NULL` context and confirms each returns
  `SG_ERROR_NULL_POINTER` (or a safe no-op / `NULL` return, for the
  `void`/pointer-returning ones) instead of crashing — `SG_ERROR_NULL_
  POINTER` existed as a declared error code from the start but had never
  actually been exercised by anything before this. `sizeof(SGFrame)`
  matched between `gcc` and `rustc` (40 bytes) with no manual padding in the
  header, confirming the `#[repr(C)]` layout really is what the header
  claims. Build/run instructions are in a comment at the top of that file.

Result codes and quality levels are declared in the C header as `int32_t`,
not a C `enum` — a C enum's underlying type is implementation-defined, while
the Rust side is `#[repr(i32)]`, a fixed 4-byte signed integer. `int32_t` is
the type actually guaranteed to match on both sides.

**Not done:** container/interaction fields on `SGGlassElement` (grouping is
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

The roadmap's Phase 4 also calls for a **visual regression framework**
(reference/candidate/diff images, tested against a fixed scene set) —
**done**: `examples/visual_regression.rs` (a fully headless EGL-pbuffer
capture tool, no window at all — the same proven pattern as `c_smoke`, just
in Rust) renders 8 fixed named scenes (varying backdrop, geometry, light/
dark tint, and a deliberately-overlapping pair to exercise the glass-on-
glass stack response) to `tests/visual_regression/candidates/`.
`scripts/visual_regression.sh` diffs each against a committed golden image
in `tests/visual_regression/golden/` using `magick compare -metric RMSE`
(deliberately not `-metric AE` — on this environment's ImageMagick build,
7.1.2 Q16-HDRI, `AE`'s pixel count is wildly inflated in HDRI mode, e.g.
reporting >500M "differing pixels" on a 1.02M-pixel image; `RMSE`'s
normalized fraction is sane and was cross-checked against a known real
change). Verified both directions: confirmed 0% RMSE against the current
renderer, and confirmed a deliberately-reintroduced regression (temporarily
reverting the Phase 8.1 fix) was correctly caught at 2.9–6.5% RMSE on
exactly the affected scenes and 0% on the unaffected ones, before reverting
back. Run it with `scripts/visual_regression.sh` (`--update` to promote new
candidates to golden after reviewing a diff by eye).

**Not done:** the roadmap's full test-scene list (dark backdrop, gradients,
high-frequency text, animated geometry — this only covers the 4 existing
photo backdrops plus light/dark tint and overlap) and the "material
parameters + GPU/backend metadata" side of what the roadmap asks to store
alongside each capture.

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

## Phase 6 — Windows (WinUI 3 + ANGLE) integration (started, partially verified)

No Windows dev machine exists in this project's environment. Per the
roadmap's own rule ("don't assume compilation equals visual success"),
that means Phase 6 can't be marked done from here — but GitHub Actions'
`windows-latest` runners are real Windows, and `.github/workflows/windows.yml`
uses them as the actual verification step instead of guessing.

**Verified locally (Linux, mingw cross-compile):**
`cargo build --release --lib --target x86_64-pc-windows-gnu` (after
installing `rust-std-static-x86_64-pc-windows-gnu` + `mingw64-gcc` via
`dnf`) produces a real PE32+ DLL. `x86_64-w64-mingw32-objdump -p` on it
confirms all 9 `sg_*` C ABI functions are present in the export table.
This is a cross-build, not the real MSVC target, so it only proves the
crate's dependency graph and FFI surface are Windows-portable at all — see
below for what actually runs on Windows.

**Verified on real Windows (GitHub Actions `windows-latest`):**
`.github/workflows/windows.yml` has two jobs:
- `native-build-and-test`: builds `x86_64-pc-windows-msvc` natively (the
  real target triple), runs `cargo test --lib` there, and re-confirms the
  export table via `dumpbin /exports`. Check the workflow's latest run for
  current status — first attempt failed at `cargo test --lib` because
  `gtk4` (a dev-dependency needed only by the Linux-only
  `examples/gtk_glarea*.rs`) doesn't build without pkg-config/system GTK
  headers, which don't exist on that runner; fixed by scoping `gtk4` to
  `[target.'cfg(target_os = "linux")'.dev-dependencies]` in `Cargo.toml`,
  since it was never going to be needed on Windows anyway.
- `c-abi-smoke`: builds `c_smoke/main.c` (the same file
  `scripts/verify_backends.sh` already proves byte-identical on Linux)
  with MSVC, linked against Google ANGLE's EGL/GLESv2 via vcpkg, and runs
  it. This is the first time SparkGlass-rendered pixels have gone through
  ANGLE on any platform — check the workflow's latest run for whether it
  passed; the vcpkg ANGLE build is built from source and takes a while.

**Not done, deliberately unverified:** `platform/windows/` has a WinUI 3 +
`SwapChainPanel` + ANGLE host scaffold (`SparkGlassPanel.h`/`.cpp`) sketching
the actual product integration — **not built, not run, not wired into any
project**. Its own README explains exactly what's uncertain (mainly the
EGL `SwapChainPanel` native-window property-set keys, which are an
ANGLE-internal, version-sensitive contract). Don't upgrade this to "done"
without a real Windows machine, Visual Studio, the Windows App SDK, and an
actual screenshot — same rule as every other platform here.

---

## Phase 8.1 — Material Style System (done)

Per `SparkGlass_ROADMAP.md`'s Phase 8.1: "stop treating every glass surface
as the same material profile." Found and fixed a real instance of exactly
that in every example scene.

**The bug:** every demo scene (`sandbox.rs`, `gtk_glarea.rs`,
`ffi_smoke.rs`) computed one `preset(GlassStyle::Regular, ...)` (or, in
`ffi_smoke.rs`, one set of hand-written literals) and reused it verbatim for
*both* the large panel (`GlassStyle::Regular`) and the small pill
(`GlassStyle::Control`). The two surfaces only ever differed in size, never
in material — directly contradicting Phase 8.1's own acceptance criterion
("a small control pill and a large navigation bar should not look like the
same shader merely scaled to different dimensions").

**The fix:**
- `preset()` in `src/glass.rs` now buckets `GlassStyle::Control` with
  `GlassStyle::Thin` (frost 6, tint 0.15) instead of with `Regular`/
  `Navigation` (frost 16, tint 1.0) — matching the Figma reference's own
  size-class split (`Frost - Regular` vs `Frost - Large`; see
  `docs/references/figma-liquid-glass/README.md`). This is evidence-based,
  not a guess: 6 and 16 are exactly the two frost values the Figma file
  uses, just previously assigned to the wrong buckets in our code.
- Every example now calls `preset()` once per surface's actual style
  instead of once for the whole scene, and the FFI smoke test's hand-written
  `SGGlassElement` literals were updated to match (`frost_radius: 16.0`,
  `tint_opacity: 1.0` for the Regular panel; the Control pill's literals
  already happened to match the corrected values).

**Verified:** `scripts/verify_backends.sh` still passes — `sandbox`,
`ffi_smoke`, and `gtk_glarea` remain byte-identical to each other (the fix
was applied identically to all three, so cross-backend parity holds, just
at corrected values). Confirmed with a real screenshot that the panel and
pill are now visibly different — the panel properly frosted/white-tinted,
the pill staying much clearer — instead of looking identical apart from
size.

**Not done:** `refraction_strength`/`depth`/`dispersion`/lighting are still
flat across all styles. Per the Figma evidence this is *correct* (those
values were identical across every instance checked in the reference file),
so this isn't a known gap — just noting it wasn't re-derived from scratch,
only frost/tint were. The container/interaction fields Phase 9 will need,
and any per-style tuning beyond frost/tint (e.g. distinct highlight
strength or edge response per style, per the roadmap's Phase 8.1 "tune per
style" list), remain undone.

---

## Phase 8.2 — Optical Calibration (done)

The roadmap is explicit about method here: *"reference scene + parameter
sweep + side-by-side comparison + human visual evaluation. Do not rely on
one developer's memory."* That's a human step by design — so what's done is
the tooling that step needs, not a calibration decision.

`examples/parameter_sweep.rs` renders the same 640×498 panel over
`assets/image1.jpg` used by the Figma "Regular - Large" reference, sweeping
one parameter at a time across several values, writing each to
`tests/parameter_sweep/<parameter>/<value>.png` (gitignored — exploratory
candidates, not golden references). Currently sweeps the two parameters
flagged unresolved in `docs/references/figma-liquid-glass/README.md`:
`refraction_strength` (1 through 24 — deliberately excludes Figma's raw
`70`, since that README already shows the math for why that's implausible
as a direct value) and `tint_opacity` (0.15 through 1.0, bracketing both
current buckets and Figma's stored `Opacity: 25`). Spot-checked the output:
the sweep produces clearly, usefully different results per value (low
refraction is a subtle, nearly flat edge; high values show a pronounced 3D
lens edge with visible chromatic fringing) — confirmed it's actually useful
for comparison, not just technically running.

**How the human step actually happened:** rather than only the static
sweep-image comparison above, `src/main.rs` grew an interactive config mode
(`cargo run`, `P` to switch profiles, arrow keys to tune every parameter
live against a real photo backdrop, `S` to save) so the team could tune
by eye in real time instead of reading sweep frames side by side — a
different instantiation of the roadmap's "reference scene + parameter
sweep + side-by-side comparison + human visual evaluation" method, not a
shortcut around it. The team confirmed the resulting values (the
`PROFILES` in `src/main.rs`, unchanged since before this session's
refactors — see git history: `refraction: 2.0` has been constant since the
profiles were first introduced) are correct for now. Product code isn't
limited to these 3 profiles — `preset()`/`GlassMaterial`/`GlassOptics` are
fully open for a custom per-surface tuning the same way.

---

## Phase 8.3 — SDF Geometry Fidelity (done, via existing architecture)

The roadmap's desired data ("R → distance, G/B → nearest-boundary normal")
is one possible encoding; it explicitly says "the exact encoding can
differ." `glass.frag`'s `sd_shape()` already computes the superellipse
distance analytically per-pixel (not from a baked texture), and
`layer_glass()` derives the surface normal from `sd_shape`'s own gradient
(central difference, `n2` in the code) rather than storing one. This
satisfies every acceptance point the roadmap lists — consistent
superellipse geometry, stable normals, smooth corners, no resize
discontinuity (nothing is baked/cached across frames), stable refraction
near boundaries — and does it with less state than a texture-based SDF
would need. No code change was required for this sub-phase; it was already
correct, just not previously credited as satisfying Phase 8.3 explicitly.

## Phase 8.4 — Refraction & Lensing Fidelity (done, via existing architecture)

`layer_glass()`'s model is exactly the roadmap's: SDF distance (`sd`) + SDF
normal (`n2`) + material depth (`u_depth`, the bezel) + refraction amount
(`u_refraction`) → a `refract()`-based sampling displacement → the
refracted backdrop. `surface_height()`/`surface_slope()` give the squircle
bezel profile the roadmap asks for ("stronger displacement near
boundaries, calmer center") — `t = 0` at the edge, `t = 1` at the plateau.
Depth is already a per-surface value (not global). No texture tearing or
edge instability has been observed in any of the visual regression goldens
or the parameter sweep across `refraction_strength = 1..24`. Also no code
change required; already satisfied.

## Phase 8.5 — Adaptive Material Response (done — new infrastructure)

Added `GlassMaterial.adaptive_response` (`0.0` in every `preset()`, a true
no-op — see verification below) and a shader-side `local_busyness` proxy in
`layer_glass()`: `length(stacked - blurred)`, i.e. how much the sharp and
frosted backdrop samples disagree at this pixel. This needs no extra
texture fetch (both samples already exist for the frost mix) and never
leaves the GPU — no `glReadPixels()` feedback loop, per the roadmap's GPU
rule. When `adaptive_response > 0`, it scales extra headroom on the edge
light and inner-glow rim terms (up to +30%/+25% at `1.0`), implementing the
roadmap's "busy backgrounds may require stronger separation."

**Verified:**
- `scripts/visual_regression.sh` → all 9 goldens still 0.0000%–0.0002%
  RMSE after this change (the tool's own pre-existing noise floor — see
  `pill_only_bg1`), proving the new code path is inert at every shipped
  preset's default.
- `examples/parameter_sweep.rs` gained an `adaptive_response` sweep
  (0 → 1 over `assets/image1.jpg`); `magick compare -metric RMSE` between
  the `0` and `1` frames shows a real, non-zero, edge-localized difference
  (~0.03% of the full frame — small because that particular backdrop photo
  is already soft-focus, so there isn't much sharp/blur disagreement for
  the proxy to react to; the mechanism is confirmed working, just subtle on
  this reference image).

**Not done:** nobody has picked a non-zero default for any shipped style
yet — same "tooling built, calibration is a human decision" situation as
8.2 originally was. The `adaptive_response` sweep exists for whoever does
that next.

## Phase 8.6 — Surface Response (done, via existing architecture)

The roadmap wants the highlight system to be more than "a 1px white
border," with independent directional highlight contributions. `glass.frag`
already has three distinct mechanisms, each reading different geometry:
a directional specular band (`lit`/`band`, driven by `u_light_angle`), a
1px edge-facing highlight (`edge_light`, stronger on the side away from the
light), and two independent inner-glow rims (`glow_top`/`glow_bottom`,
Figma's own "two inner shadows" — genuinely two separate `sd_shape` calls
with opposite Y offsets, not one term mirrored). These were already
independent before this phase; Phase 8.5's `local_busyness` boost now
modulates the edge light and rim terms without merging them into one pass.
No further change made.

## Phase 8.7 — Shadow & Depth (done)

`layer_shadow()` already varies shadow offset/opacity by light/dark tint
mode, and `GlassLighting.shadow_strength` is a genuine per-surface knob
(wired to `u_shadow`, tunable live in `cargo run`'s config mode). What the
roadmap adds beyond that — "busy backgrounds may require stronger
separation" — is the same backdrop-adaptive signal built for 8.5
(`local_busyness`), applied to the same rim/edge terms that give glass its
felt separation from the backdrop; a literal drop-shadow-blur change would
need an extra backdrop sample outside the shape's coverage region, which
wasn't needed to satisfy the roadmap's stated acceptance criteria. Depth
(`u_depth`) already varies per style via `preset()`.

## Phase 8.8 — Clear Material Dimming (done — new infrastructure)

Added `GlassMaterial.clear_dimming` (`0.0` in every `preset()`, a true
no-op) and a shader term in `layer_glass()`, applied to the transmitted
backdrop color right after tint and before highlights are added: it
darkens `col` in proportion to `clear_dimming * col`'s own local
luminance. Deliberately placed *before* highlights so specular/rim light
isn't dimmed along with the backdrop — those are surface reflections, not
part of what's being seen through the glass. Style-dependent by
construction: it's a per-`GlassMaterial` field, so a product opts in for
`Thin`/low-tint styles specifically rather than getting it globally.

**Verified:** same golden-regression proof as 8.5 (inert at `0.0`).
`examples/parameter_sweep.rs` gained a `clear_dimming` sweep (0 → 1);
`magick compare -metric RMSE` between the `0` and `1` frames shows a large,
clearly-visible ~17% difference — the mechanism works and is strong enough
to matter at the high end (also strong enough to look like "just a dark
panel" past roughly `0.6` on this photo, so a real product default should
land well below `1.0`).

**Not done:** no shipped style has a non-zero default yet — same
"infrastructure built, calibration is a human decision" pattern as 8.5.

## Phase 8.9 — Composition Reference Model (done — documentation alignment)

The roadmap's semantic order and the actual code now line up explicitly
(passes are fused per-fragment in `glass.frag`'s `main()`/`layer_glass()`,
which the roadmap explicitly allows — "passes may be fused if fidelity is
preserved"):

```text
Roadmap layer                          glass.frag
─────────────────────────────────────  ─────────────────────────────────
Original backdrop                      u_scene / u_scene_blur samples
Adaptive cast shadow                   layer_shadow() (not yet backdrop-
                                        adaptive itself — see 8.7)
Optional clear-material dimming        clear_dimming term (8.8)
Blur / scattering                      frost mix (stacked/blurred)
Refracted / lensed backdrop            layer_glass()'s refract() sampling
Adaptive tint / transmission           layer_tint() (not yet backdrop-
                                        adaptive itself — see 8.5 note)
Highlight field A                      specular band (lit/band)
Highlight field B                      edge_light
Adaptive rim / surface response        glow_top/glow_bottom, boosted by
                                        local_busyness (8.5)
Interaction illumination               NOT YET WIRED — see GlassSurface
                                        .interaction (roadmap Phase 9.5)
Native foreground content              host-owned; drawn after SparkGlass,
                                        never sampled by it
```

No code changes were needed for this sub-phase — it was a documentation
task to make the mapping explicit, not a gap in the renderer.

---

## Phase 9.1 — Explicit Glass Containers (done)

`GlassGroup` existed since the Phase 1 audit but was genuinely dead data —
`GlassScene::groups` was always empty and nothing ever read it. This gives
it real behavior:

- `GlassScene::add_group(name, style, surface_ids)` declares a container.
  Validates that `surface_ids` is non-empty, that `name` is unique within
  the scene, and that every id already exists in `self.surfaces` —
  returns `GroupError` otherwise rather than silently accepting a
  malformed group.
- `GlassScene::group(name)` / `group_surfaces(name)` look a group and its
  current member surfaces back up. Membership is by stable `id`, not
  vector index — the same reason `surface_at`'s hit-testing already used
  ids, so a group survives `bring_to_front` reordering `self.surfaces`.
  A member id with no matching surface (removed from the scene after the
  group was declared) is silently skipped by `group_surfaces` rather than
  treated as an error — group membership tracks intent, not scene
  lifetime.
- `GlassScene::bring_group_to_front(name)` — the group-level equivalent of
  the existing single-surface `bring_to_front`: moves every member to the
  front together, preserving their relative order, so a panel with an
  attached control pill can be dragged as one cluster without the pill
  getting left behind underneath it.
- `GlassScene::apply_group_style(name)` — forces every member's `.style`
  to match the group's, making "these elements belong to one material
  region" (the roadmap's own phrase for 9.1) enforceable rather than a
  convention the caller has to remember on every surface by hand. Extends
  Phase 8.1's per-style material bucketing to the container level.

**Verified:** 5 new unit tests in `src/glass.rs` (`cargo test --lib`) cover
validation (empty/duplicate/unknown-id rejection), membership lookup
including the removed-surface case, group-level reordering (including the
no-op case for an unknown group name), and style enforcement touching only
members. `scripts/verify_backends.sh` and `scripts/visual_regression.sh`
both still pass unchanged — this phase only added new pure-Rust scene-model
API, it didn't touch any renderer or shader, so there was nothing for
those to catch, but they were run anyway rather than assumed clean.

**Not done:** nothing in `src/main.rs` or the examples actually creates a
group yet — the panel+pill demo scenes are still two independent surfaces.
Wiring a real demo up is straightforward now that the API exists, but
wasn't done here to avoid touching `src/main.rs`'s already-tuned
interactive config mode without a specific reason to.

## Phase 9.2 — Shared Sampling Regions (not started)

## Phase 9.3 — Shared / Merged SDF (not started)

The roadmap explicitly says not to require this before Phase 8's
style-level work is complete — it now is, so this is unblocked, just not
started.

## Phase 9.4 — Morphing (not started)

`GlassScene::reduced_motion` already exists and is plumbed through the FFI
frame struct, waiting for a motion system to reduce — see its doc comment
in `src/glass.rs`.

## Phase 9.5 — Interaction Illumination (not started)

`GlassSurface::interaction` already exists and is tracked (`src/main.rs`
sets it while dragging a panel) but has zero corresponding shader uniform —
see its doc comment in `src/glass.rs`. This is the most direct next step
if 9.x work continues: the state already flows through the scene model,
it just doesn't reach `glass.frag` yet.

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

## Visual reference material (new: unblocks Phase 8 calibration work)

`docs/references/` now has real calibration material, which was the missing
piece for any Phase 8 (advanced fidelity) work:

- `references/figma-liquid-glass/` — the actual Figma file (`BBTOV`) this
  project's shader constants were almost certainly calibrated against
  originally: it uses the same source photo as `assets/image1.jpg`, has a
  component literally named `Liquid Glass - Regular - Large` sized exactly
  640×498 (matching every demo scene's main glass surface), and its
  variable values match this codebase's `preset()`/`"Clear"` profile
  defaults exactly on 5 of 7 comparable parameters. Two values
  (`Refraction: 70`, `Opacity: 25`) don't transplant directly — see that
  directory's `README.md` for why, and don't act on those two numbers
  without resolving the ambiguity first. That README also has a real
  composited screenshot (not an isolated component preview) showing a
  `Liquid Glass - Dark` bar over real album art — it's visibly more
  frosted/darkened than our demo's "Clear" profile produces, which is real
  evidence our demo currently applies one flat profile to every surface
  regardless of `GlassStyle` rather than a specific wrong constant.
- `references/apple-liquid-glass/` — real macOS screenshots (Apple Music,
  System Settings) showing actual Liquid Glass in production use. Useful
  for structure and edge treatment, not for numeric calibration — see that
  directory's `README.md` for why.

Before starting any Phase 8 visual work, read both READMEs. They turn "we'd
be guessing" (the blocker noted earlier for this phase) into "we have
numbers for 5 parameters and know why 2 don't transplant yet" — a real
difference, but still not a green light to invent values for the two that
remain unresolved.

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
