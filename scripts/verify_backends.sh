#!/usr/bin/env bash
# Automates what this project's verification has been doing by hand:
# render the same "Clear" profile scene through every host integration and
# confirm the output is byte-identical, plus run the pure-C ABI smoke test.
#
# This does NOT replace an external-screenshot check of render()/present()
# changes (see docs/SparkGlass_HANDOFF.md — a same-process capture cannot
# prove a frame reached a *different* framebuffer, e.g. the host's own). Run
# this for routine regressions; take a real screenshot when touching the
# framebuffer-binding logic itself.
#
# Requires a working GL/GLES-capable display for the winit/glutin and GTK4
# examples (DISPLAY or WAYLAND_DISPLAY set) — this is not yet runnable in a
# headless CI environment without a virtual display (e.g. Xvfb).
#
# Usage: scripts/verify_backends.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target_dir="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

pass=0
fail=0

note() { printf '\n== %s ==\n' "$1"; }
ok() { printf '  OK: %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf '  FAIL: %s\n' "$1"; fail=$((fail + 1)); }

note "Building"
cargo build -p spark_glass --bins --examples --lib

note "Capturing each backend's output (headless glReadPixels, frame 5)"
SPARK_GLASS_CAPTURE="$work_dir/reference_macroquad.png" timeout 15 cargo run -p spark_glass --bin spark_glass >/dev/null 2>&1 || true
SPARK_GLASS_SANDBOX_CAPTURE="$work_dir/sandbox.png" timeout 15 cargo run -p spark_glass --example sandbox >/dev/null 2>&1 || true
SPARK_GLASS_SANDBOX_CAPTURE="$work_dir/ffi_smoke.png" timeout 15 cargo run -p spark_glass --example ffi_smoke >/dev/null 2>&1 || true
if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    SPARK_GLASS_SANDBOX_CAPTURE="$work_dir/gtk_glarea.png" timeout 15 "$target_dir/debug/examples/gtk_glarea" >/dev/null 2>&1 || true
fi

note "Comparing captures (must be byte-identical — see docs/SparkGlass_IMPLEMENTATION_STATUS.md)"
baseline="$work_dir/sandbox.png"
if [ ! -f "$baseline" ]; then
    bad "sandbox.rs did not produce a capture — nothing to compare against"
else
    for name in ffi_smoke gtk_glarea; do
        candidate="$work_dir/$name.png"
        if [ ! -f "$candidate" ]; then
            [ "$name" = "gtk_glarea" ] && [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && continue
            bad "$name: no capture produced"
            continue
        fi
        if cmp -s "$baseline" "$candidate"; then
            ok "$name matches sandbox byte-for-byte"
        else
            bad "$name DIFFERS from sandbox — this is exactly the class of regression this script exists to catch"
        fi
    done
fi

note "Pure-C ABI smoke test (no Rust in this build)"
if command -v gcc >/dev/null && [ -f /usr/include/EGL/egl.h ]; then
    c_bin="$work_dir/spark_glass_c_smoke"
    if gcc c_smoke/main.c -Iinclude -lEGL -lGLESv2 -L"$target_dir/debug" -lspark_glass -Wl,-rpath,"$target_dir/debug" -o "$c_bin" 2>"$work_dir/c_smoke_build.log"; then
        if "$c_bin" >"$work_dir/c_smoke_run.log" 2>&1 && grep -q "All checks passed" "$work_dir/c_smoke_run.log"; then
            ok "c_smoke: all checks passed"
        else
            bad "c_smoke: build succeeded but checks failed — see $work_dir/c_smoke_run.log"
            cat "$work_dir/c_smoke_run.log" >&2
        fi
    else
        bad "c_smoke: build failed — see $work_dir/c_smoke_build.log"
        cat "$work_dir/c_smoke_build.log" >&2
    fi
else
    printf '  SKIP: gcc or EGL headers not available\n'
fi

note "Unit tests"
if cargo test -p spark_glass --lib -q >"$work_dir/unit_tests.log" 2>&1; then
    ok "cargo test --lib"
else
    bad "cargo test --lib — see $work_dir/unit_tests.log"
    cat "$work_dir/unit_tests.log" >&2
fi

note "Result: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
