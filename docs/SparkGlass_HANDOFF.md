# SparkGlass — Handoff Notes

> Written mid-session as a safety checkpoint (the operator has no way to
> detect their own usage-limit percentage, so this is kept current rather
> than written only at the very end). If you're reading this, either the
> prior session ended and you're continuing cold, or it's still running and
> this is just the latest checkpoint — check git log to see which.

## Where things actually stand

Read `docs/SparkGlass_IMPLEMENTATION_STATUS.md` first — it's the living
status doc and has the real detail. This file is only the context that
doesn't fit there: what happened, what surprised us, what to watch out for.

Short version, as of `221272e`: Phases 0–5 from
`docs/SparkGlass_MASTER_ARCHITECTURE.md` §54 are implemented and
independently verified (not just "compiles"). Four separate host
integrations — a `winit`+`glutin` sandbox, the `extern "C"` ABI called
from Rust, a real GTK4 `GtkGLArea` widget, and a plain C program linked
against the compiled `.so` — render byte-identical output. Phase 8 (Apple
Material Fidelity) is fully addressed — see
`SparkGlass_IMPLEMENTATION_STATUS.md`'s Phase 8.1–8.9 sections. Phase 9 is
**started**: 9.1 (containers, `GlassGroup` is no longer dead data) and 9.5
(interaction glow) are done; 9.2/9.3/9.4 are not. Phase 6 (Windows) is
**started, not finished**: GitHub Actions `windows-latest` CI (no Windows
dev machine exists here) proves the C ABI and GL/ANGLE rendering pipeline
work on real Windows, but the actual WinUI 3 + `SwapChainPanel` product
integration is still an unverified scaffold in `platform/windows/` — see
that directory's own README for exactly what's uncertain. Phase 7
(GoosicReborn integration) hasn't started at all; it's blocked on having
access to that codebase.

`src/main.rs` (`cargo run`) also went through a full UI rework this
session, prompted directly by the team actually using it: unified demo
navigation (`DemoScene`, `[`/`]` to cycle, including an `AppleReference`
scene that renders our own glass on top of real macOS/Apple Music
screenshots from `docs/references/apple-liquid-glass/` for direct
comparison), a config panel that's real SparkGlass instead of a plain UI
box (`config_panel_glass()` + `build_glass_skin()`), and all 12 tunable
parameters exposed as sliders (not just the original 9 — Phase 8.5/8.8's
`clear_dimming`/`adaptive_response`/`ambient_reflection` are now
live-tunable). One real bug surfaced and got fixed along the way:
`ambient_reflection`'s first implementation only ever reached the
top/bottom edges and was too subtle to see — see
`SparkGlass_IMPLEMENTATION_STATUS.md`'s "Extension: ambient reflection"
section for the fix and how it was actually verified (cropped edge
comparisons, not just an overall diff number).

## Things that will trip you up if you don't know them

### 1. This directory is nested in someone else's Cargo workspace

`liquid_glass_poc/` sits inside `~/Dev/PersonalProyects/RustProjects/Learning/`,
which has its own workspace `Cargo.toml` (`members = [...]` including several
unrelated learning-exercise crates). That means:

- `cargo build`/`check` run from inside `liquid_glass_poc/` still resolve
  against the **workspace root's** `Cargo.lock` and build into the
  **workspace root's** `target/`, not a local one. `liquid_glass_poc/Cargo.lock`
  in git is essentially inert — it never actually changes when you add
  dependencies, because cargo isn't using it.
- Don't run `cargo build --workspace` unless you actually want to compile
  the unrelated sibling crates too (`hello_world`, `data_types`, etc.) — use
  `-p spark_glass` or just build from inside the crate directory without
  `--workspace`.
- Binaries end up at
  `~/Dev/PersonalProyects/RustProjects/Learning/target/debug/...`, not
  `liquid_glass_poc/target/debug/...`.

### 2. The GitHub repo was renamed mid-session

`origin` still points at `git@github-personal:AnalogicGoose/LiquidGlass.git`
(the repo's old name). GitHub redirects pushes/fetches to the real location,
`git@github-personal:AnalogicGoose/SparkGlass.git`, and prints a warning each
time — that warning is expected, not an error. Repointing the remote was
attempted once and denied by the auto-mode permission classifier (repo
repoint is treated as a risky action); it was left as-is since the redirect
works fine. If it becomes annoying, ask the user before changing it.

### 3. The `epoxy` crate is currently unusable

Needed for GL proc-address loading inside a `GtkGLArea` (the usual gtk-rs
pattern). Its `gl_generator` dependency requires `xml-rs ^0.7.0`, and both
0.7.0 and 0.7.1 are yanked with no replacement 0.7.x published — a fresh
`cargo` resolve simply cannot satisfy it. Worked around by dlopening
`libEGL.so.1` directly and calling `eglGetProcAddress` ourselves (see
`GlLoader` in `examples/gtk_glarea.rs`). If this surfaces again elsewhere,
don't re-attempt the `epoxy` crate — check crates.io first in case it's been
fixed, otherwise reuse the direct-dlopen pattern.

### 4. A same-process pixel readback does not prove a frame reached the screen

This is the single most important lesson from this session, and it cost a
lot of back-and-forth to find. `glReadPixels` reads whatever framebuffer is
*currently bound* — if your own code left the wrong framebuffer bound after
"finishing" a frame, the readback will still look perfect because it's
reading your own leftover target, not the host's real one. This exact bug
shipped and passed verification for a while (see
`SparkGlass_IMPLEMENTATION_STATUS.md` → "The render-target bug") before an
actual screenshot of a running window exposed it. If you add or change
anything in the render/present path, verify with a real external screenshot
of the actual window, not just an internal capture — see item 5.

### 5. Live-screenshot verification: what worked, and a caution about it

`spectacle -b -n -a -o <path>` (KDE, on this machine) takes a real
screenshot of the active window and was essential for finding the bug in
item 4. Substitute whatever's available on a different compositor (`grim`
on wlroots, `gnome-screenshot`, etc.).

**Caution, not a blocker:** partway through this session, a full-desktop
screenshot (`spectacle -f`, used once to sanity-check the tool itself)
revealed this machine's real, live desktop — the user was in an active
screen-shared voice call at the time. The user was asked and explicitly said
not to worry about it and to keep opening windows/taking screenshots freely
("no need to be careful whit the windows, do what you need to do"). That
permission stands for continuing this same kind of work, but a future
session picking this up cold should still default to a quick check-in before
resuming heavy GUI-window/screenshot activity, since a new session has no
way to know if that context still applies (a fresh session has no memory of
this exchange) or whether the user's situation has changed since.

**Second caution, discovered later the same session:** after many rounds of
opening a test window and killing it with `pkill -9` (dozens, across
`sandbox`/`gtk_glarea`/`gtk_glarea_overlay`), *every* windowed example
started hanging indefinitely on startup — both `winit`+`glutin` and GTK4,
two completely unrelated windowing stacks, failing identically. Headless
tools (`examples/visual_regression.rs`, `c_smoke`, both using an EGL
pbuffer with no on-screen surface at all) kept working perfectly throughout
— proving this isn't a code regression, and confirming the render logic
itself was never in question. KWin was still alive with no errors in
`journalctl`. The strong suspicion: `SIGKILL` never gives the client a
chance to cleanly release its Wayland `xdg_toplevel`/surface, and enough
abrupt kills in a row left the compositor's Wayland state degraded for
*new* toplevel surfaces specifically, from this client. Never got to root
cause it further or confirm a fix (a session logout/re-login would very
likely clear it, but that's the user's call, not something to do
unilaterally). **Update: it cleared on its own** — after switching to only
using the auto-exiting capture path (`SPARK_GLASS_SANDBOX_CAPTURE=<path>`,
which calls `event_loop.exit()`/`std::process::exit(0)` itself once it's
captured a frame, needing no `pkill` at all) and just waiting, a retest of
`sandbox` worked normally again, byte-identical to golden, and the full
`scripts/verify_backends.sh` (all three windowed backends plus `c_smoke`)
passed clean afterward. So it was transient, not a lasting compositor
problem — but the lesson stands regardless: **prefer closing test windows
normally over `pkill -9`, and if `-9` is truly needed, don't lean on it
dozens of times in one session.** If a windowed example starts hanging for
no code-side reason, this is the first thing to suspect — try the headless
tools first to confirm the code itself is fine, then just wait a bit and
retry the windowed one before assuming something is actually broken.

## Who owns what, architecturally

**GPT is the project's architect/orchestrator** — it wrote
`SparkGlass_MASTER_ARCHITECTURE.md` originally, and it wrote
`SparkGlass_ROADMAP.md` (in a separate ChatGPT conversation the user ran
alongside this session, seeded with this session's Figma findings and a
question list — see git history around the `SparkGlass_ROADMAP.md` commit).
Claude's role here has been implementation/verification: building what the
architecture calls for, and pressure-testing it (the render-target bug, the
GL-state experiment, the Figma calibration pull) rather than designing it.
If a roadmap/architecture question comes up that's really a product/design
decision rather than an implementation one, the right move is routing it
back through the user to GPT, the same way this session did for Phase 8 —
not improvising an answer solo.

## Suggested next steps, roughly in priority order

1. **Nothing is currently broken** — the last commit (`221272e` as of this
   writing) left everything building clean and passing every regression
   check described in `SparkGlass_IMPLEMENTATION_STATUS.md`, including all
   three windowed backends. Safe to pick up from any angle below.
   `scripts/visual_regression.sh` and `scripts/verify_backends.sh` both
   pass; run them after any shader or renderer change before trusting it.
   The Windows CI workflow's `c-abi-smoke` job was still finishing its
   first real pass (after fixing a missing `d3dcompiler_47.dll`) when this
   was written — check its latest run before assuming it's green.
2. **`docs/SparkGlass_ROADMAP.md`'s Phase 8 (Apple Material Fidelity) is
   now fully addressed** — see `SparkGlass_IMPLEMENTATION_STATUS.md`'s
   Phase 8.1–8.9 sections for the detail per sub-phase. Short version: 8.1
   (per-style material bucketing) and 8.2 (optical calibration) were done
   earlier and confirmed correct by the team directly, via `cargo run`'s
   interactive config mode rather than only the static sweep tool.
   8.3/8.4/8.6/8.7/8.9 turned out to already be satisfied by the existing
   shader architecture (documented, no code change). 8.5 (Adaptive
   Material Response) and 8.8 (Clear Material Dimming) are new:
   `GlassMaterial.adaptive_response`/`.clear_dimming`/`.ambient_reflection`,
   all `0.0` (a proven no-op — see the goldens) in every shipped preset,
   with real effects once turned on. **`ambient_reflection`'s first cut
   was actually broken** — reused Figma's top/bottom-only inner-shadow
   geometry, so a panel with colorful backdrop content to its *sides*
   showed nothing no matter the slider value, and the team caught it
   directly ("compare this with the reference... in our case it's not
   [happening]"). Fixed into a real omnidirectional rim band — see that
   section in the status doc for how it was actually re-verified (cropped
   left/right edges separately, not just one diff number). **Picking a
   non-zero shipped default for any of the three is still an open human
   decision** — don't guess at one; ask. `src/main.rs`'s config panel now
   exposes all three as live sliders, including over real Apple reference
   photos (`DemoScene::AppleReference`), specifically so that decision can
   be made by looking, not by reading sweep PNGs.
3. **Figma reference material exists** at `docs/references/`: the actual
   file the shader constants were almost certainly calibrated against
   (same source photo as `assets/image1.jpg`, components named/sized to
   match our demo scenes exactly), including one real *composited* render
   (`goosic_mockup_composited_dark_bar.jpg` — glass over real album art, not
   an isolated component preview) that's more informative than the isolated
   ones. Read `docs/references/figma-liquid-glass/README.md` before
   touching any shader constant. The Figma MCP server needs authentication
   each new session (`mcp__plugin_figma_figma__authenticate`) and
   `get_design_context` doesn't work here (needs Figma desktop, unavailable
   on this Linux/browser-only setup) — `get_metadata`/`get_screenshot`/
   `get_variable_defs` all work fine without it. Two lessons learned pulling
   it, documented in that README: composited (not isolated) screenshots need
   a shared parent frame in the Figma file, and `get_variable_defs` only
   returns locally-overridden values, never the full inherited set.
4. **Phase 1 (freeze semantics) audit — 2 rounds done, not finished.**
   Round 1: `GlassScene::new` was fabricating a hardcoded demo `GlassGroup`
   nothing ever read — fixed. Round 2 found `GlassMaterial::saturation`/
   `brightness`/`contrast`, `GlassOptics::surface_curvature`, and
   `GlassSurface::interaction` with zero corresponding shader uniform;
   `saturation`/`brightness`/`contrast` got wired for real during Phase 8
   (a neutral-by-default color-grade stage in `glass.frag`) — the other two
   are still genuinely dead and documented as such. `reduced_transparency`
   confirmed genuinely wired;
   `reduced_motion` confirmed not (no motion system exists yet for it to
   affect). Backdrop/TextureHandle-beyond-FFI/RenderTarget haven't had this
   treatment yet — same method: grep every field's write-sites against its
   read-sites before trusting what a doc comment claims.
5. ~~Automate the regression checks~~ — done, see `scripts/verify_backends.sh`
   and `scripts/visual_regression.sh` (the latter is a *different* check —
   catches unintended changes to the renderer's own output over time, not
   cross-backend divergence; see `SparkGlass_IMPLEMENTATION_STATUS.md`'s
   Phase 4 section for the distinction and why `-metric RMSE` is used
   instead of `-metric AE`, which is unreliable on this environment's
   ImageMagick build).
6. **§37 GL state contract / roadmap §13** — the overlay experiment
   (`examples/gtk_glarea_overlay.rs`) is one data point on GTK4/Mesa/Wayland.
   Worth checking whether a heavier native-widget scene (more widgets,
   popups, scrolling) still composites cleanly, and eventually whether
   anything on the WinUI side has a similar concern once that platform is
   reachable. The roadmap is explicit: don't freeze this from one GTK test.
7. **Phase 6 (Windows) — what's left specifically.** CI now covers: native
   MSVC build + unit tests + C-ABI export check
   (`native-build-and-test`), and building/linking/running `c_smoke.exe`
   against the cdylib through real ANGLE (`c-abi-smoke`). What's NOT
   covered and needs an actual Windows machine + Visual Studio: everything
   in `platform/windows/` (the WinUI 3 `SwapChainPanel` + ANGLE host
   scaffold) — it has never been compiled, only reviewed. That
   directory's own README lists exactly what's uncertain (mainly the EGL
   `SwapChainPanel` native-window property-set keys, an ANGLE-internal,
   version-sensitive contract). Don't mark Phase 6 "done" until that
   scaffold actually builds and produces a real screenshot.
8. **Phase 9 — what's left specifically.** 9.1 (Explicit Glass Containers:
   `GlassScene::add_group`/`group_surfaces`/`bring_group_to_front`/
   `apply_group_style`) and 9.5 (Interaction Illumination: `u_interaction`
   in `glass.frag`, driven by `interaction_energy()`) are done and tested.
   9.2 (Shared Sampling Regions), 9.3 (Shared/Merged SDF — the roadmap
   says this is unblocked now that Phase 8 is done, just not started), and
   9.4 (Morphing) are real, substantial architecture work, not started at
   all.
9. **Phase 7 — GoosicReborn integration.** Not started, not attempted,
   blocked on having access to that codebase at all. This is likely the
   single highest-value next step once it's unblocked, since it's the
   actual point of the whole project — everything else has been building
   toward a library that's ready to be integrated, not the integration
   itself.
10. **Phase 10 (performance)** — not started. The roadmap says wait until
    material behavior is settled enough to measure meaningfully, which,
    with Phase 8 done, is arguably now.

## How to verify a change is real, not just "it compiled"

The pattern used throughout this session, worth repeating for any future
change to the renderer or its host integrations:

1. `cargo build -p spark_glass --bins --examples --lib` (no
   `--workspace`) — clean, no warnings.
2. `cargo test -p spark_glass --lib` — passes.
3. Run the affected example(s) with `SPARK_GLASS_SANDBOX_CAPTURE=<path>` set,
   `cmp` the PNG against a known-good prior capture (or against another
   backend's capture, since they're all supposed to match byte-for-byte).
4. If the change touches `render()`/`present()`/framebuffer binding
   specifically: also take a **real external screenshot** of the actual
   running window (see item 4/5 above) — the internal capture alone is not
   sufficient proof, as this session found out the hard way.
5. For ABI (`src/ffi/`) changes: rebuild `c_smoke/` against the new header
   and rerun it — a Rust-side caller isn't sufficient proof the layout is
   really C-compatible.
