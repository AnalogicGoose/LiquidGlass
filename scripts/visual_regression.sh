#!/usr/bin/env bash
# Phase 4 (docs/SparkGlass_ROADMAP.md) visual regression check.
#
# Renders the fixed scene set (examples/visual_regression.rs) and compares
# each against tests/visual_regression/golden/<name>.png using ImageMagick's
# `compare`. This catches *unintended* changes to the renderer's output —
# it says nothing about whether that output is correct against real Liquid
# Glass (see docs/references/figma-liquid-glass/README.md for that,
# separate, question).
#
# Usage:
#   scripts/visual_regression.sh              # compare against golden, report pass/fail
#   scripts/visual_regression.sh --update      # after reviewing a diff by eye and
#                                               # deciding the new output is correct,
#                                               # promote candidates to the new golden

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

golden_dir="tests/visual_regression/golden"
candidate_dir="tests/visual_regression/candidates"
diff_dir="tests/visual_regression/diffs"
# RMSE normalized-error percentage allowed before a scene is reported FAIL.
# Nonzero because driver/float rounding can shift pixels by a level or two
# even when nothing meaningful changed. This project's rendering is
# otherwise fully deterministic (see scripts/verify_backends.sh, which
# checks byte-identical output across backends), so in practice this stays
# at 0.0000% until something real changes.
threshold_percent=0.05

mkdir -p "$golden_dir" "$candidate_dir" "$diff_dir"

echo "== Rendering scenes =="
cargo run --example visual_regression

if [ "${1:-}" = "--update" ]; then
    echo "== Updating golden images from candidates =="
    cp "$candidate_dir"/*.png "$golden_dir"/
    echo "Done. Review the diff with 'git diff --stat -- $golden_dir' before committing."
    exit 0
fi

pass=0
fail=0
new=0

echo "== Comparing against golden =="
for candidate in "$candidate_dir"/*.png; do
    name="$(basename "$candidate")"
    golden="$golden_dir/$name"

    if [ ! -f "$golden" ]; then
        echo "  NEW: $name (no golden yet — run with --update once you've reviewed it)"
        new=$((new + 1))
        continue
    fi

    diff_path="$diff_dir/${name%.png}_diff.png"
    # Output looks like "3917.12 (0.0597714)" — a raw RMSE value, then the
    # normalized (0..1) error fraction in parentheses, which is what we
    # actually want. (Deliberately not -metric AE: on this ImageMagick
    # build — 7.1.2 Q16-HDRI — AE's pixel count is wildly inflated in HDRI
    # mode, e.g. reporting >500M "differing pixels" on a 1.02M-pixel image.
    # RMSE's normalized fraction is sane and matches a plain byte diff.)
    rmse_output="$(magick compare -metric RMSE -fuzz 2% "$golden" "$candidate" "$diff_path" 2>&1 || true)"
    normalized="$(echo "$rmse_output" | sed -n 's/.*(\(.*\))/\1/p')"
    if ! [[ "$normalized" =~ ^[0-9.]+([eE][+-]?[0-9]+)?$ ]]; then
        echo "  FAIL: $name — could not parse compare output: $rmse_output"
        fail=$((fail + 1))
        continue
    fi

    percent="$(awk -v n="$normalized" 'BEGIN { printf "%.4f", n * 100 }')"
    within_threshold="$(awk -v p="$percent" -v t="$threshold_percent" 'BEGIN { print (p <= t) ? 1 : 0 }')"

    if [ "$within_threshold" = "1" ]; then
        rm -f "$diff_path"
        echo "  OK: $name (${percent}% RMSE)"
        pass=$((pass + 1))
    else
        echo "  FAIL: $name — ${percent}% RMSE, see $diff_path"
        fail=$((fail + 1))
    fi
done

echo
echo "Result: $pass passed, $fail failed, $new new (no golden yet)"
[ "$fail" -eq 0 ]
