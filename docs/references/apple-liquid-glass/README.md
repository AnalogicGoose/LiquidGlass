# Real Apple Liquid Glass Screenshots

Provided by the user 2026-09-15 (macOS, real running apps — not mockups).

- `apple_music_home_sidebar.jpg` / `apple_music_home_sidebar_crop.jpg` —
  Apple Music, dark mode, Home tab. Full-height glass sidebar over the app's
  own dark background.
- `apple_music_artist_page_sidebar.jpg` — same sidebar, different scroll
  position (Metro Boomin artist page), colorful album art visible to the
  right of the sidebar (not behind it).
- `macos_settings_glass_menu.jpg` — macOS System Settings, a floating glass
  panel (search bar, account row, nav list with "General" selected) over the
  desktop/other windows, with individually-glassy rounded-rect list rows
  inside it (About, Software Update, Storage, AppleCare & Warranty, AirDrop
  & Continuity, AutoFill & Passwords).

## What these are useful for, and what they aren't

All four are **dark-mode OS chrome over low-contrast, mostly-dark
backdrops**. That makes them good for:

- Confirming general structure: native window controls (traffic lights)
  stay sharp/un-blurred and separate from the glass, exactly per this
  project's "native controls stay native" rule.
- Corner rounding and edge treatment: a very subtle rim highlight, no hard
  outline.
- Confirming Apple really does compose *multiple small glass elements*
  together (the Settings list rows) rather than treating each control as
  fully independent — supporting evidence for this project's future
  container/grouping direction (architecture doc §13), even though nothing
  here was measured precisely enough to calibrate against.

They are **not** useful for calibrating refraction/dispersion/frost
strength numerically — the backdrops behind the glass in all four shots are
too flat and dark to show strong optical distortion, so there's nothing
with enough contrast to measure. For that, see `../figma-liquid-glass/`,
which has actual named components with numeric parameter values attached.
