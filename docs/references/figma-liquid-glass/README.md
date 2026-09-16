# Figma Liquid Glass Reference — "BBTOV"

Source: https://www.figma.com/design/mZFSuamMyCdd9ZzZduXZbV/BBTOV?node-id=49-40801
(page "GLASSS", file key `mZFSuamMyCdd9ZzZduXZbV`)

Provided by the user 2026-09-15. This is almost certainly the exact file
`src/glass.frag`'s Spanish comments refer to when they say values are
"calibrado contra el render de Figma" (calibrated against the Figma render):
the file uses **the same source photo** as `assets/image1.jpg` (verified —
see below), and contains a component named exactly `Liquid Glass - Regular -
Large` sized exactly **640×498**, which is exactly the size of the main
glass surface in every one of this project's demo scenes
(`main.rs`, `examples/sandbox.rs`, `examples/ffi_smoke.rs`,
`examples/gtk_glarea.rs`). That is not a coincidence.

Pulled via the Figma MCP server (`get_metadata`, `get_screenshot`,
`get_variable_defs`) — note `get_design_context` did not work in this
environment (it requires a live selection in the Figma **desktop** app,
which is not available here; Linux, browser-only Figma access). The three
tools above worked without it.

## What's in this directory

- `liquid_glass_regular_large_640x498.png` — node `242:901`, "Liquid Glass -
  Regular - Large", isolated on a flat neutral backdrop (Figma's own
  component-spec preview, not composited over real content).
- `liquid_glass_clear_regular_520x456.png` — node `250:891`, "Liquid Glass -
  Clear", same flat-backdrop isolation.
- `liquid_glass_clear_light_160x160.png` — node `251:57`, "Liquid Glass -
  Clear/Light", a small badge/icon-sized variant.
- `goosic_mockup_composited_dark_bar.png` — node `103:8098`, a **real
  composited render**: a "Goosic" (GoosicReborn?) music-app mockup with an
  actual `Liquid Glass - Dark` bar sitting over real album-art covers. This
  is not an isolated component — it's the glass genuinely compositing real
  content, found by screenshotting a *parent* frame that contains both the
  background and the glass layer as children (isolated child screenshots
  only show a flat gray placeholder backdrop; a shared parent frame renders
  the real composite). See "The dark-style tint/frost gap" below — this one
  image is more informative than all the isolated component renders
  combined.
- `goosic_dark_bar_closeup.jpg` — tighter crop of the bar above.
- `our_clear_profile_control_bar.jpg` — the equivalent element from our own
  renderer (`examples/sandbox.rs`'s pill "Control" surface, "Clear"
  profile), cropped from a verified capture, for direct comparison.

## Extracted variable values vs. our code

Figma variable names are as returned by `get_variable_defs` (values are
plain numbers — Figma renders them as 0–100 UI sliders for the 0–1-range
parameters, and as degrees/pixels for the others).

### `Liquid Glass - Regular - Large` (242:901, 640×498) vs. `preset(GlassStyle::Regular)` in `src/glass.rs`

| Parameter | Figma | Ours | Match |
|---|---|---|---|
| Frost | 16 | `frost_radius: 16.0` | **exact** |
| Splay | 20 | `splay: 0.2` | **exact** |
| Light Angle | 0° | `angle_degrees: 0.0` | **exact** |
| Depth | 30 | `depth: 30.0` | **exact** |
| Dispersion | 20 | `dispersion: 0.2` | **exact** |
| Opacity | 25 | `tint_opacity: 1.0` | unclear — see note below |
| Refraction | 70 | `refraction_strength: 2.0` | unclear — see note below |

### `Liquid Glass - Clear` (250:891, 520×456) vs. the "Clear" profile in `main.rs` (`PROFILES[0]`)

| Parameter | Figma | Ours | Match |
|---|---|---|---|
| Frost | 6 | `frost` param: 6.0 | **exact** |
| Splay | 20 | `splay` param: 0.2 | **exact** |
| Light Angle | 0° | `light_angle` param: 0.0 | **exact** |
| Depth | 30 | `depth` param: 30.0 | **exact** |
| Dispersion | 20 | `dispersion` param: 0.2 | **exact** |
| Refraction | 70 | `refraction` param: 2.0 | unclear — see note below |

**Five of six comparable parameters match exactly** (after the obvious
÷100 unit conversion for the 0–1-range ones) on *both* size classes. This
is strong, concrete confirmation that `frost_radius`, `splay`,
`angle_degrees`, and `dispersion` — and by extension the general approach —
were genuinely calibrated against this file, not guessed. That's good news,
not a bug list.

### The two that don't transplant directly: Refraction and Opacity

`Refraction: 70` appears identically on every instance checked (both size
classes, both style variants) — so whatever it represents, it isn't a
per-style tuning knob, it's closer to a constant in Figma's own effect UI.
Plugging `70` directly into our `u_refraction` uniform does not work: our
shader computes

```glsl
float thickness = u_refraction * u_depth * mix(1.0, 0.12, stack_response);
```

With `depth = 30` (which *does* match), `u_refraction = 70` gives
`thickness = 2100`, roughly **35× larger** than the working value
(`2.0 * 30 = 60`) this codebase actually uses and was visually verified
against this same design (see `docs/SparkGlass_IMPLEMENTATION_STATUS.md`
for the byte-identical cross-backend verification, which used exactly these
"Clear" profile numbers). A thickness of 2100 on a 640×498 panel would
produce extreme, almost certainly broken-looking distortion. The most
likely explanation is that Figma's "Refraction" is a 0–100 UI slider feeding
some *other* formula inside whatever effect plugin built this file — not the
same parameter as our `u_refraction` — so **do not change `refraction` to
70** on the strength of this number alone. Confirming what it actually
controls would need the plugin's own shader/effect source, which isn't
available here.

`Opacity: 25` was only found once (on the Regular-Large instance; not shown
on the Clear instances, meaning it's either inherited or unused there). It's
unclear which of our uniforms, if any, it corresponds to — `tint_opacity`
is the obvious guess by name, but our Regular preset uses `1.0` and this
would suggest `0.25`, while our separately-tuned "Clear" *profile* already
uses `0.15` for the same concept and matches the reference well everywhere
else. Treat this one as unresolved rather than acting on it.

## The dark-style tint/frost gap (found via the real composited render)

`goosic_mockup_composited_dark_bar.png` / `goosic_dark_bar_closeup.jpg` show
a `Liquid Glass - Dark` bar over real, colorful album covers (Luis Miguel,
Michael Jackson, Elton John × Dua Lipa, etc.). Two things are immediately
visible that aren't visible in any isolated-component render:

1. **The backdrop is moderately blurred but still clearly legible** — text
   ("LUIS MIGUEL"), faces, and album-art shapes all survive the frost. No
   sign of extreme distortion or displacement at the bar's edges — album
   covers don't look "bent," just blurred. This is more evidence against
   `Refraction: 70` being a large geometric-displacement multiplier like our
   `u_refraction` — a real ~35×-too-thick lens would visibly warp this
   content, and it doesn't.
2. **The bar is visibly, noticeably darkened/desaturated** relative to the
   uncovered album art around it — a real gray-toned dimming, not a subtle
   sheen.

Compare that to `our_clear_profile_control_bar.jpg` — the same *kind* of
element (a pill-shaped "Control" surface) from our own renderer, using the
"Clear" profile (`frost: 6.0`, `tint_opacity: 0.15`). Ours barely darkens
the backdrop at all; the flowers underneath stay near full brightness and
saturation. Figma's dark bar looks meaningfully more frosted *and* more
tinted than ours.

**This is likely not really about `Refraction` at all — it's that our demo
scenes apply one flat "Clear" profile to every surface regardless of
`GlassStyle`.** `main.rs`'s `apply_tuning()` overwrites every surface's
material/optics/lighting with the same profile values every frame,
regardless of whether that surface is `GlassStyle::Regular` or
`GlassStyle::Control` — so the pill and the large panel currently always
look identical apart from size. Figma's file clearly does *not* do this: it
has distinct `Frost - Large` / `Frost - Regular` variables, a `Liquid Glass
- Dark` component family with its own tuning, and (going by this composited
render) meaningfully stronger frost/tint than our "Clear" profile at
whatever size/style this bar actually is. Root cause candidates, in order of
how likely they seem from the evidence above:

- Our "Clear" profile (`frost: 6`, `tint: 0.15`) may simply be tuned for a
  *lighter* style than what this dark bar represents, and is being misapplied
  to every surface in the demo instead of only the ones it was meant for.
- `tint_opacity` in general may be under-strength for the "Dark" family
  specifically — nothing here rules that out, but nothing confirms an exact
  replacement number either, since `get_variable_defs` on this bar's own
  node only returned `Light Angle: 0` (the rest are inherited from a
  component definition this API doesn't expose in full).

Either way: **this is a real, visually-evidenced gap, not a guess** — but
the fix isn't "set tint_opacity to X," it's first deciding whether the demo
should vary its glass parameters by `GlassStyle` at all (it currently
doesn't), and then tuning the "Dark"/larger styles against renders like this
one specifically, the same way "Clear" was tuned against the isolated
component renders.

## What the small "Clear/Light" variant suggests

`Liquid Glass - Clear/Light` (160×160) only overrides `Light Angle: -45°`
and `Dispersion: 0` relative to whatever it inherits — both different from
every other instance checked (`0°` and `0.2` respectively). That's evidence
Figma's library gives small badge-sized glass elements their own distinct
tuning rather than just scaling down the same numbers, which lines up with
this project's own `GlassStyle` enum already distinguishing `Thin` /
`Control` / `Navigation` etc. — but this specific small-variant combination
(angle -45°, zero dispersion) isn't currently replicated by any preset in
`src/glass.rs`. Worth a look if/when a small-badge `GlassStyle` gets tuned.

## How to pull more from this file

```text
get_metadata(nodeId, fileKey)        # structure — no desktop app needed
get_screenshot(nodeId, fileKey)      # isolated render of one node
get_variable_defs(nodeId, fileKey)   # bound variable values for one node
```

`get_design_context` (richer output: code hints, all design tokens at once)
requires the Figma **desktop** app with the node actually selected there —
unavailable in this environment (Linux, browser-only Figma). If a future
session has desktop Figma available, re-pulling with `get_design_context`
on `242:901` would likely surface the "Opacity"/"Refraction" ambiguity
above directly (e.g. as a documented effect-layer blend mode) instead of
leaving it as a guess.

**Important lesson on `get_screenshot`:** screenshotting an isolated glass
node (e.g. `242:901` directly) renders it against a flat placeholder
backdrop, not real content — even though the file does place real photos
near several glass components. But screenshotting a **shared parent frame**
that has both the background and the glass as children (e.g. `103:8098`,
"Frame 1") *does* render the true composite. Check `get_metadata` for a
common ancestor before assuming a composited reference isn't available —
it's worth searching for one before concluding "isolated only."
`get_variable_defs` has the mirror-image limitation: called on an instance
node, it only returns variables *locally overridden on that exact node*, not
the full set inherited from whatever component it's an instance of — so an
instance can look like it has "only 1 property" when it really has 7, most
of them just inherited. There isn't a tool in this set that resolves the
full effective/inherited value set from a plain instance node.
