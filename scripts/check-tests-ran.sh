#!/bin/bash
# Proves every integration test file ran, from the JUnit report nextest wrote:
# a green run where a file silently stopped running must not pass.
# Usage: check-tests-ran.sh REPORT. Exit 0 every file ran, 1 one did not,
# 3 the report is missing or unreadable, or a file's #![cfg] is not understood.
set -uo pipefail

report=${1:?usage: check-tests-ran.sh REPORT}
# CHECK_ROOT and CHECK_OS stand in for the repo and the host in the self-test.
cd "${CHECK_ROOT:-$(dirname "$0")/..}" || exit 3
cannot() { echo "CANNOT CHECK: $1" >&2; exit 3; }

case "${CHECK_OS:-$(uname -s)}" in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    MINGW* | MSYS* | CYGWIN* | Windows_NT) os=windows ;;
    *) cannot "unknown OS $(uname -s)" ;;
esac

# Whether a whole-file #![cfg(...)] includes this OS. Only the forms the repo
# uses are understood; anything else cannot be judged here.
cfg_includes_os() {
    case "$1" in
        'target_os = "linux"') [ "$os" = linux ] ;;
        'target_os = "macos"') [ "$os" = macos ] ;;
        windows | 'target_os = "windows"') [ "$os" = windows ] ;;
        unix) [ "$os" != windows ] ;;
        *) return 2 ;;
    esac
}

[ -f "$report" ] || cannot "no test report at $report"
total=$(sed -n 's/.*<testsuites name="nextest-run" tests="\([0-9][0-9]*\)".*/\1/p' "$report" | head -n 1)
[ -n "$total" ] || cannot "$report has no nextest run totals"
suites=$(sed -n 's/.*<testsuite name="\([^" ]*\)" tests="\([0-9][0-9]*\)".*/\1 \2/p' "$report")
[ -n "$suites" ] || cannot "$report lists no test suites"

files=0
missing=()
skipped=()
for file in rust/*/tests/*.rs; do
    [ -f "$file" ] || cannot "no integration test files under rust/*/tests"
    files=$((files + 1))
    crate=$(basename "$(dirname "$(dirname "$file")")")
    suite="$crate::$(basename "$file" .rs)"
    count=$(awk -v s="$suite" '$1 == s { print $2 }' <<<"$suites")
    [ -n "$count" ] && [ "$count" -gt 0 ] && continue

    cfg=$(sed -n 's/^#!\[cfg(\(.*\))\]$/\1/p' "$file" | head -n 1)
    if [ -n "$cfg" ]; then
        cfg_includes_os "$cfg"
        case $? in
            1) skipped+=("$suite ($cfg)"); continue ;;
            2) cannot "$file: cannot tell whether #![cfg($cfg)] applies to $os" ;;
        esac
    fi
    missing+=("$suite")
done

nsuites=$(wc -l <<<"$suites" | tr -d ' ')
echo "$total tests in $nsuites suites; $((files - ${#missing[@]} - ${#skipped[@]})) of $files test files ran"
for s in ${skipped[@]+"${skipped[@]}"}; do echo "  not built on $os: $s"; done

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    {
        echo "### Tests on $os: $total in $nsuites suites"
        echo
        echo "| Suite | Tests |"
        echo "| --- | ---: |"
        awk '{ printf "| `%s` | %s |\n", $1, $2 }' <<<"$suites"
        for s in ${skipped[@]+"${skipped[@]}"}; do echo "| \`$s\` | not built on $os |"; done
        for s in ${missing[@]+"${missing[@]}"}; do echo "| \`$s\` | **did not run** |"; done
    } >>"$GITHUB_STEP_SUMMARY"
fi

if [ ${#missing[@]} -gt 0 ]; then
    for s in "${missing[@]}"; do echo "FAIL: test file did not run: $s" >&2; done
    exit 1
fi
