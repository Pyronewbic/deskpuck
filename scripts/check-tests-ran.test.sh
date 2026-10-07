#!/bin/bash
# Self-test for check-tests-ran.sh: every failure path must fail, against a
# throwaway repo and hand-made reports. Exit 0 when every case behaves.
set -uo pipefail

guard="$(cd "$(dirname "$0")" && pwd)/check-tests-ran.sh"
work=$(mktemp -d) || exit 3
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/repo/rust/alpha/tests" "$work/repo/rust/beta/tests"
echo '#[test] fn a() {}' >"$work/repo/rust/alpha/tests/one.rs"
echo '#[test] fn b() {}' >"$work/repo/rust/beta/tests/two.rs"
printf '%s\n' '//! Linux only.' '#![cfg(target_os = "linux")]' >"$work/repo/rust/beta/tests/desk.rs"

report() {
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    echo "<testsuites name=\"nextest-run\" tests=\"9\" skipped=\"0\" failures=\"0\" errors=\"0\">"
    for suite in "$@"; do
        echo "    <testsuite name=\"${suite% *}\" tests=\"${suite#* }\" skipped=\"0\" errors=\"0\" failures=\"0\">"
        echo "    </testsuite>"
    done
    echo '</testsuites>'
}

failures=0
expect() {
    local want=$1 name=$2 os=$3 file=$4
    env -u GITHUB_STEP_SUMMARY CHECK_ROOT="$work/repo" CHECK_OS="$os" "$guard" "$file" >"$work/out" 2>&1
    local got=$?
    if [ "$got" -ne "$want" ]; then
        echo "FAIL: $name: exit $got, want $want" >&2
        sed 's/^/    /' "$work/out" >&2
        failures=$((failures + 1))
    fi
}

report 'alpha::one 1' 'beta::two 2' 'beta::desk 1' >"$work/all.xml"
expect 0 "every file ran" Linux "$work/all.xml"
report 'alpha::one 1' 'beta::two 2' >"$work/no-desk.xml"
expect 0 "a Linux-only file is excused on macOS" Darwin "$work/no-desk.xml"
expect 0 "a Linux-only file is excused on Windows" MINGW64_NT-10.0 "$work/no-desk.xml"
expect 1 "a Linux-only file must run on Linux" Linux "$work/no-desk.xml"
report 'alpha::one 1' 'beta::desk 1' >"$work/no-two.xml"
expect 1 "a file that did not run fails" Darwin "$work/no-two.xml"
report 'alpha::one 0' 'beta::two 2' 'beta::desk 1' >"$work/zero.xml"
expect 1 "a file that ran zero tests fails" Linux "$work/zero.xml"
expect 3 "a missing report cannot be checked" Linux "$work/absent.xml"
echo 'not a report' >"$work/garbage.xml"
expect 3 "an unreadable report cannot be checked" Linux "$work/garbage.xml"
report >"$work/empty.xml"
expect 3 "a report with no suites cannot be checked" Linux "$work/empty.xml"
expect 3 "an unknown OS cannot be checked" Plan9 "$work/all.xml"

: >"$work/summary.md"
GITHUB_STEP_SUMMARY="$work/summary.md" CHECK_ROOT="$work/repo" CHECK_OS=Darwin "$guard" "$work/no-two.xml" >/dev/null 2>&1
grep -q '`beta::two` | \*\*did not run\*\*' "$work/summary.md" \
    || { echo "FAIL: the summary does not name the file that did not run" >&2; failures=$((failures + 1)); }

printf '%s\n' '#![cfg(feature = "slow")]' >"$work/repo/rust/alpha/tests/odd.rs"
expect 3 "a #![cfg] it cannot judge cannot be checked" Linux "$work/all.xml"

[ "$failures" -eq 0 ] && echo "check-tests-ran.sh: all 12 cases behave"
[ "$failures" -eq 0 ]
