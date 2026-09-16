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

Short version: Phases 0–5 from `docs/SparkGlass_MASTER_ARCHITECTURE.md` §54
are implemented and independently verified (not just "compiles"). Four
separate host integrations — a `winit`+`glutin` sandbox, the `extern "C"`
ABI called from Rust, a real GTK4 `GtkGLArea` widget, and a plain C program
linked against the compiled `.so` — render byte-identical output. Phase 6
(Windows/WinUI/ANGLE) has not been started because there is no Windows
environment available here to build or verify it against — don't write
unverified Windows integration code; it would violate the project's own
"don't assume compilation equals visual success" rule.

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

## Suggested next steps, roughly in priority order

1. **Nothing is currently broken** — the last commit (`2c85697` as of this
   writing) left everything building clean and passing every regression
   check described in `SparkGlass_IMPLEMENTATION_STATUS.md`. Safe to pick up
   from any angle below.
2. **Phase 1 (freeze semantics) audit — started, not finished.** One gap was
   found and fixed: `GlassScene::new` was fabricating a hardcoded demo
   `GlassGroup` nothing ever read. Go through the rest of the master doc's
   Phase 1 checklist (Scene / Backdrop / Container / GlassElement / Material
   / TextureHandle / RenderTarget) the same way — grep for who actually
   reads each field before trusting a doc comment about it.
3. ~~Automate the regression checks~~ — done, see `scripts/verify_backends.sh`.
4. **§37 GL state contract** — the overlay experiment
   (`examples/gtk_glarea_overlay.rs`) is one data point on GTK4/Mesa/Wayland.
   Worth checking whether a heavier native-widget scene (more widgets,
   popups, scrolling) still composites cleanly, and eventually whether
   anything on the WinUI side has a similar concern once that platform is
   reachable.
5. **Phase 6 (Windows)** only becomes attemptable with actual access to a
   Windows machine or CI runner to build and screenshot-verify against — do
   not write unverified WinUI/ANGLE integration code and mark it "done."
6. **Phase 8 (advanced fidelity)** — shared blur/SDF across containers,
   adaptive luminance, etc. — is real, scoped, verifiable work but hasn't
   been started; the "Still Open" section of the master doc's §55 has the
   open questions that would need answering first for some of it (container
   semantics, backdrop scope).

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
