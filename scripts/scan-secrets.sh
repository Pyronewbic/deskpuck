#!/bin/bash
# Scans commits for secrets with gitleaks; used by the pre-push hook and CI.
# Arguments are git revisions ("A..B", or "B --not --remotes=origin"); none
# means the whole history. Exit 0 clean, 1 secrets found, 3 the scan could not
# run or did not cover every commit: an incomplete scan never counts as clean.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

cannot() { echo "CANNOT CHECK: $1" >&2; exit 3; }

command -v gitleaks >/dev/null || cannot "gitleaks not found (brew install gitleaks)"
[ $# -gt 0 ] || set -- HEAD

count=$(git rev-list --count "$@") || cannot "could not list the commits in: $*"
[ "$count" -eq 0 ] && exit 0
# gitleaks scans only commits with a text change (not merges, renames or binaries).
patched=$(git log --no-merges -p -U0 --format='tformat:@C' "$@" |
    LC_ALL=C awk '$0 == "@C" { seen = 0; next } /^@@/ && !seen { n++; seen = 1 } END { print n + 0 }') ||
    cannot "could not read the changes in: $*"

# gitleaks exits 0 when git itself fails, so its ERR lines count as failure too.
# Leaks exit 2 so that 1 can only mean gitleaks failed.
output=$(gitleaks git --no-banner --no-color --redact --exit-code 2 --log-opts="$*" . 2>&1)
status=$?
grep -vE ' INF ' <<<"$output" >&2
if [ $status -eq 2 ]; then
    echo "Secrets found in: $*" >&2
    exit 1
elif [ $status -ne 0 ] || grep -q ' ERR ' <<<"$output"; then
    cannot "gitleaks failed (exit $status)"
elif ! grep -q " $patched commits scanned" <<<"$output"; then
    cannot "gitleaks did not scan all $patched changed commits"
fi
echo "No secrets in $count commits."
