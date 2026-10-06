#!/bin/bash
# Every check that must pass before code leaves this machine; CI runs it too.
# Exit 0 all passed, 1 a check failed, 3 a check could not run (missing tool,
# advisory database unreachable): unknown is never treated as passed.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

failed=()
fail() { failed+=("$1"); echo "FAIL: $1" >&2; }
cannot() { echo "CANNOT CHECK: $1" >&2; exit 3; }

for tool in cargo cargo-deny; do
    command -v "$tool" >/dev/null || cannot "$tool not found (brew install rustup cargo-deny)"
done

(cd rust && cargo fmt --check) || fail "cargo fmt --check"
(cd rust && cargo clippy --quiet --locked --all-targets) || fail "cargo clippy"
(cd rust && cargo test --quiet --locked) || fail "cargo test"

# Fetch separately so an unreachable network is "could not check", not a finding.
# cargo-deny reads every platform's crates, not only the ones this host built.
(cd rust && cargo deny --log-level error fetch db) || cannot "advisory database could not be fetched"
(cd rust && cargo fetch --locked --quiet) || cannot "crates could not be fetched"
(cd rust && cargo deny --log-level error --offline check) || fail "cargo deny (advisories, licenses, sources)"

if [ "$(uname)" = Darwin ]; then
    if scripts/build-rust.sh >/dev/null; then
        swift build -c release --quiet || fail "swift build"
    else
        fail "scripts/build-rust.sh"
    fi
fi

if [ ${#failed[@]} -gt 0 ]; then
    echo "${#failed[@]} check(s) failed: ${failed[*]}" >&2
    exit 1
fi
echo "All checks passed."
