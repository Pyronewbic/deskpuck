#!/bin/bash
# Self-test for release.sh --artifacts: the CI files are copied to dist/ only
# when their checksums and attestations verify. Runs against a throwaway repo
# with a stub gh. Exit 0 when every case behaves, 1 otherwise.
# Each check is a single-quoted condition that check() evals later.
# shellcheck disable=SC2016,SC2034
set -uo pipefail

release="$(cd "$(dirname "$0")" && pwd)/release.sh"
work=$(mktemp -d) || exit 3
trap 'rm -rf "$work"' EXIT
repo="$work/repo"
mkdir -p "$repo/scripts" "$work/bin"
cp "$release" "$repo/scripts/"
echo 'VERSION="9.8.7"' >"$repo/scripts/make-app.sh"
git -C "$repo" init -q && git -C "$repo" remote add origin https://github.com/owner/proj.git &&
    git -C "$repo" -c user.name=t -c user.email=t@t commit -q --allow-empty -m init || exit 3
head=$(git -C "$repo" rev-parse HEAD)

# The stub serves one passing run of this commit; STUB_ATTEST and
# STUB_CORRUPT pick what goes wrong.
cat >"$work/bin/gh" <<'STUB'
#!/bin/bash
case "$1 $2" in
    "api repos/owner/proj") echo true ;;
    "run list") echo 42 ;;
    "api repos/owner/proj/actions/runs/42")
        echo ".github/workflows/release-artifacts.yml $(git rev-parse HEAD) success owner/proj" ;;
    "run download")
        for pair in linux:Deskpuck-9.8.7-linux-x86_64.tar.gz windows:Deskpuck-9.8.7-windows-x86_64.zip \
            windows:Deskpuck-9.8.7-setup.exe; do
            dir="dist/ci/deskpuck-${pair%%:*}" file=${pair#*:}
            mkdir -p "$dir" && printf '%s' "$file" >"$dir/$file"
            (cd "$dir" && shasum -a 256 "$file" >"$file.sha256")
            if [ -n "${STUB_CORRUPT:-}" ]; then printf x >>"$dir/$file"; fi
        done ;;
    "attestation verify") echo "$*" >>"$STUB_LOG"; exit "${STUB_ATTEST:-0}" ;;
    *) echo "stub gh: unexpected $*" >&2; exit 99 ;;
esac
STUB
chmod +x "$work/bin/gh"

failures=0
check() {
    if ! eval "$2"; then
        echo "FAIL: $1" >&2
        sed 's/^/    /' "$work/out" >&2
        failures=$((failures + 1))
    fi
}
run() {
    rm -rf "$repo/dist" "$work/log"
    (cd "$repo" && PATH="$work/bin:$PATH" GH_TOKEN=stub STUB_LOG="$work/log" "$@" scripts/release.sh --artifacts) \
        >"$work/out" 2>&1
}
copied() { find "$repo/dist" -maxdepth 1 -name 'Deskpuck-*' 2>/dev/null | grep -c .; }

run env; status=$?
check "verified files exit 0" '[ $status -eq 0 ]'
check "verified files are copied to dist/" '[ "$(copied)" -eq 3 ]'
check "each file's attestation is verified" '[ "$(grep -c . "$work/log")" -eq 3 ]'
want="--repo owner/proj --source-digest $head --deny-self-hosted-runners"
want="$want --signer-workflow owner/proj/.github/workflows/release-artifacts.yml"
check "the attestation must come from this repo's workflow at this commit" \
    '[ "$(grep -cF -- "$want" "$work/log")" -eq 3 ]'

run env STUB_ATTEST=1; status=$?
check "a file without a valid attestation exits 1" '[ $status -eq 1 ]'
check "it says so" 'grep -q "has no attestation from release-artifacts.yml" "$work/out"'
check "and copies nothing" '[ "$(copied)" -eq 0 ]'

run env STUB_CORRUPT=1; status=$?
check "a file that does not match its checksum exits 1" '[ $status -eq 1 ]'
check "before its attestation is checked" '[ ! -s "$work/log" ] && [ "$(copied)" -eq 0 ]'

if [ $failures -ne 0 ]; then
    exit 1
fi
echo "release.sh: all cases behave"
