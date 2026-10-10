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

for tool in cargo cargo-deny cargo-nextest shellcheck; do
    command -v "$tool" >/dev/null || cannot "$tool not found (brew install rustup cargo-deny cargo-nextest shellcheck)"
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

group "Shell scripts"
shellcheck -x scripts/*.sh .githooks/pre-push packaging/linux/*.sh || fail "shellcheck"
# makepkg reads the PKGBUILD's variables (SC2034), sets pkgdir, srcdir and CARCH (SC2154),
# and runs package() with errexit, so a failed cd stops it (SC2164).
shellcheck --shell=bash -e SC2034,SC2154,SC2164 packaging/aur/deskpuck-bin/PKGBUILD packaging/aur/deskpuck-bin/deskpuck.install ||
    fail "shellcheck (AUR package)"
endgroup

group "Linux installer"
packaging/linux/install.test.sh || fail "packaging/linux/install.sh self-test"
endgroup

# A pre-push hook run from a worktree exports GIT_DIR, and a self-test that inherits it
# re-initialises the real repository. Those that make a throwaway repo run with GIT_DIR
# naming a decoy, which must come back byte for byte.
decoy=$(mktemp -d) || cannot "no temporary folder for the decoy repository"
trap 'rm -rf "$decoy"' EXIT
env -u GIT_DIR -u GIT_WORK_TREE -u GIT_COMMON_DIR -u GIT_INDEX_FILE git init -q "$decoy" ||
    cannot "could not make the decoy repository"
decoy_files() { (cd "$decoy/.git" && find . -type f -exec cksum {} + | sort); }
isolated() {
    local before status
    before=$(decoy_files)
    GIT_DIR="$decoy/.git" "$@"
    status=$?
    if [ "$(decoy_files)" != "$before" ]; then
        echo "$1 wrote to the repository named by GIT_DIR" >&2
        return 1
    fi
    return $status
}

if [ "$(uname)" = Darwin ]; then
    group "Release script"
    isolated scripts/release.test.sh || fail "release.sh self-test"
    endgroup
fi

# Not on Windows: its sha256sum writes binary-mode "*file" sums the stub's
# SHA256SUMS would carry, and the AUR flow never runs there.
case "$(uname)" in
    MINGW* | MSYS* | CYGWIN*) ;;
    *)
        group "AUR package script"
        isolated scripts/aur-update.test.sh || fail "aur-update.sh self-test"
        endgroup
        ;;
esac

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
