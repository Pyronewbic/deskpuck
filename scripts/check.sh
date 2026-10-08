#!/bin/bash
# Every check that must pass before code leaves this machine; CI runs it too.
# Exit 0 all passed, 1 a check failed, 3 a check could not run (unknown is never passed).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

failed=()
fail() { failed+=("$1"); echo "FAIL: $1" >&2; }
cannot() { echo "CANNOT CHECK: $1" >&2; exit 3; }
group() { if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::group::$1"; else echo "== $1"; fi; }
endgroup() { if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::endgroup::"; fi; }

for tool in cargo cargo-deny cargo-nextest; do
    command -v "$tool" >/dev/null || cannot "$tool not found (brew install rustup cargo-deny cargo-nextest)"
done

group "Format"
(cd rust && cargo fmt --check) || fail "cargo fmt --check"
endgroup

group "Lints"
(cd rust && cargo clippy --quiet --locked --all-targets) || fail "cargo clippy"
endgroup

group "Tests"
# Removed first, so a report left by an earlier run can never stand in for this one.
report=rust/target/nextest/check/junit.xml
rm -f "$report"
(cd rust && cargo nextest run --locked --no-tests=fail --profile check)
tests_ran=$?
[ $tests_ran -eq 0 ] || fail "cargo nextest"
(cd rust && cargo test --doc --locked --quiet) || fail "doctests"
endgroup

group "Linux installer"
packaging/linux/install.test.sh || fail "packaging/linux/install.sh self-test"
endgroup

if [ "$(uname)" = Darwin ]; then
    group "Release script"
    scripts/release.test.sh || fail "release.sh self-test"
    endgroup
fi

group "AUR package script"
scripts/aur-update.test.sh || fail "aur-update.sh self-test"
endgroup

group "Every test file ran"
scripts/check-tests-ran.test.sh || fail "check-tests-ran.sh self-test"
if [ $tests_ran -ne 0 ] && [ ! -f "$report" ]; then
    echo "Skipped: the tests did not build, which already failed the check."
else
    scripts/check-tests-ran.sh "$report"
    case $? in
        0) ;;
        3) cannot "the test report could not be read" ;;
        *) fail "a test file did not run" ;;
    esac
fi
endgroup

group "Advisories, licenses and sources"
# Fetch separately so an unreachable network is "could not check", not a finding.
# cargo-deny reads every platform's crates, not only the ones this host built.
(cd rust && cargo deny --log-level error fetch db) || cannot "advisory database could not be fetched"
(cd rust && cargo fetch --locked --quiet) || cannot "crates could not be fetched"
(cd rust && cargo deny --log-level error --offline check) || fail "cargo deny (advisories, licenses, sources)"
endgroup

if [ "$(uname)" = Darwin ]; then
    group "Mac app build"
    if scripts/build-rust.sh >/dev/null; then
        swift build -c release --quiet || fail "swift build"
    else
        fail "scripts/build-rust.sh"
    fi
    endgroup
fi

if [ ${#failed[@]} -gt 0 ]; then
    echo "${#failed[@]} check(s) failed: ${failed[*]}" >&2
    exit 1
fi
echo "All checks passed."
