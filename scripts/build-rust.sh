#!/bin/bash
# Build the Rust core as a static library for the Mac app, then check that it
# exports every function the C header declares.
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null; then
    echo "cargo not found: install Rust (https://rustup.rs) and try again" >&2
    exit 2
fi

(cd rust && cargo build --release -p deskpuck-ffi)

lib=rust/target/release/libdeskpuck_ffi.a
header=rust/deskpuck-ffi/include/deskpuck.h
if [ ! -f "$lib" ]; then
    echo "build did not produce $lib" >&2
    exit 1
fi

declared=$(grep -oE '\bdp_[a-z_]+\(' "$header" | tr -d '(' | sort -u)
if [ -z "$declared" ]; then
    echo "no functions found in $header" >&2
    exit 1
fi
# nm exits non-zero for archive members without symbols; judge by its output instead.
symbols=$(nm -gU "$lib" 2>/dev/null || true)
if [ -z "$symbols" ]; then
    echo "could not read the symbols of $lib" >&2
    exit 1
fi
exported=$(printf '%s\n' "$symbols" | awk '{print $NF}' | sed 's/^_//' | grep -E '^dp_' | sort -u || true)
missing=$(comm -23 <(echo "$declared") <(echo "$exported"))
if [ -n "$missing" ]; then
    echo "$lib does not export: $missing" >&2
    exit 1
fi
echo "Built $lib ($(echo "$declared" | wc -l | tr -d ' ') functions exported)"
