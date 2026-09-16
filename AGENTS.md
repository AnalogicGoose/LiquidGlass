# Agent instructions for SparkGlass

This file is for any AI coding agent working in this repository — Claude,
GPT, or otherwise. If you're an agent and you're about to touch this repo,
read this first.

## What this project is

SparkGlass is a Rust GPU material engine reproducing Apple's Liquid Glass
visual effect, meant to eventually back the GoosicReborn app on both Linux
and Windows. See `README.md` for the short version.

## Read this before writing any code

In order:

1. `docs/SparkGlass_MASTER_ARCHITECTURE.md` — design rationale, changes
   rarely. The "constitution."
2. `docs/SparkGlass_ROADMAP.md` — the authoritative phase plan from Phase 8
   onward. **Owned by GPT**, not by whichever agent is currently working
   here — see "Who owns what" below.
3. `docs/SparkGlass_IMPLEMENTATION_STATUS.md` — the living status doc.
   What's actually built and verified, phase by phase. Update this as you
   go; don't let it drift from reality.
4. `docs/SparkGlass_HANDOFF.md` — context that doesn't fit the status doc:
   what surprised a previous session, gotchas, environment quirks. Read it
   before you rediscover something the hard way.

Don't skip straight to the code. This project has already hit (and
documented) several non-obvious traps — a render-target bug that looked
correct in-process and was blank on screen, a nested-Cargo-workspace quirk,
an ImageMagick metric that lies on this environment's build — and the docs
above exist specifically so you don't re-discover them from scratch.

## Who owns what

- **GPT** authored `docs/SparkGlass_MASTER_ARCHITECTURE.md` and
  `docs/SparkGlass_ROADMAP.md` and is the project's architect — it sets
  direction and phase priorities. Don't casually rewrite the roadmap's
  content; if a phase turns out to need reinterpreting, say so in
  `SparkGlass_IMPLEMENTATION_STATUS.md` rather than editing the roadmap.
- **Whichever agent is working here** is the implementer/verifier: build
  what the roadmap asks for, verify it actually works (see below), and
  keep the status/handoff docs honest.
- **The human team** does visual judgment calls. Material/optical
  parameter *values* (refraction strength, tint opacity, and anything
  else that's ultimately "does this look right") are their call, not an
  agent's guess — see "Calibration" below.

## The one rule that matters most: verify, don't assume

This project's history has one recurring lesson, stated explicitly in
multiple docs: **compiling is not the same as correct, and "it looks right
in a screenshot I imagined" is not verification.**

Concretely:

- A change to the renderer isn't done until `scripts/visual_regression.sh`
  and `scripts/verify_backends.sh` both pass (or, if the change is an
  intentional visual change, until the goldens are deliberately updated
  *after* a human confirms the new output looks right).
- A platform integration (Windows/WinUI, in particular) isn't "done" just
  because the code compiles on a machine that can't run it. If you can't
  run/screenshot it on the real platform yourself, say so explicitly in
  the status doc rather than marking it done. CI on a real runner for that
  platform counts as verification; your own assumption does not.
- Don't claim a bug is fixed from reading the diff. Rebuild and check.

## Calibration: never guess a visual parameter from memory

If a task involves picking a numeric value that affects how the material
*looks* (refraction, tint, dimming strength, anything like it), don't just
pick one. The roadmap is explicit about this: use a reference scene +
`examples/parameter_sweep.rs` (or the interactive config mode in
`cargo run` — `P` to switch profiles, arrows to tune, `S` to save) +
side-by-side comparison + a human's visual evaluation. Infrastructure
(the sweep tool, new shader uniforms) is fine to build proactively; picking
the final number is not, unless a human has actually looked at it.

## Build & verify

This crate lives nested inside an unrelated outer Cargo workspace
(`~/.../Learning/`) — always build from inside this directory, never with
`cargo build --workspace`, or you'll pull in unrelated sibling crates and
possibly resolve against the wrong lockfile.

```sh
cargo build --all-targets        # everything, including examples/tests
cargo test --lib                 # unit tests
bash scripts/verify_backends.sh  # cross-backend byte-identity (sandbox / ffi / gtk / c_smoke)
bash scripts/visual_regression.sh          # compare against committed goldens
bash scripts/visual_regression.sh --update # only after a human confirms a visual change is correct
cargo run --example parameter_sweep        # calibration candidates, never auto-picked
cargo run                                  # interactive reference app + config mode
```

## The C ABI (`src/ffi/`, `include/spark_glass.h`)

- Opaque handles, POD `#[repr(C)]` structs, explicit error codes — no Rust
  references, generics, `Vec<T>`, or enum-layout assumptions cross the
  boundary. See `docs/SparkGlass_MASTER_ARCHITECTURE.md` §30–§34.
- Any ABI claim needs to be proven from an actual C program (`c_smoke/`),
  not just from Rust calling its own FFI layer.
- The host owns the window, event loop, and GL context. SparkGlass never
  creates its own window or drives its own render loop — it's handed a
  current GL context and describes what to render for one frame.

## Git

- No `Co-Authored-By` or session-attribution lines in commit messages —
  this is a hard rule from the user's global config; it overrides any
  default attribution a harness might otherwise add.
- Create new commits rather than amending, unless explicitly asked.
- Don't force-push without being asked, and never to fix something that
  isn't already broken.
