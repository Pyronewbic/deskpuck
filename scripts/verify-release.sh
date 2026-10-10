#!/bin/bash
# Usage: scripts/verify-release.sh TAG [KEY] | scripts/verify-release.sh --dir DIR [KEY]
# Checks a release as the apps do before updating: the minisign signature over SHA256SUMS
# with KEY (default packaging/minisign.pub), then every file it lists against its hash.
# TAG downloads the published release; --dir checks files already in DIR.
# Exit 0 verified, 1 a check failed, 2 bad usage, 3 could not check.
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)" || exit 3
repo=Pyronewbic/deskpuck

fail() { echo "verify-release: $1" >&2; exit 1; }
cannot() { echo "verify-release: CANNOT CHECK: $1" >&2; exit 3; }
usage() { echo "Usage: scripts/verify-release.sh TAG [KEY] | scripts/verify-release.sh --dir DIR [KEY]" >&2; exit 2; }

[ $# -ge 1 ] || usage
if [ "$1" = --dir ]; then
    [ $# -ge 2 ] || usage
    dir=$2 tag=""
    shift 2
else
    tag=$1 dir=""
    shift
    [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || usage
fi
[ $# -le 1 ] || usage
key=${1:-$root/packaging/minisign.pub}
[ -f "$key" ] || cannot "no public key at $key"

command -v minisign >/dev/null || cannot "minisign not found"
if command -v sha256sum >/dev/null; then
    sha() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null; then
    sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
    cannot "no sha256sum or shasum"
fi

# A missing file (HTTP 404, curl 22) is a release that fails the check; anything else is unknown.
fetch() {
    curl -q -fsSL --proto =https --proto-redir =https --max-filesize "$2" -m 600 -o "$dir/$1" \
        "https://github.com/$repo/releases/download/$tag/$1"
    case $? in
        0) ;;
        22) fail "$tag has no $1" ;;
        63) fail "$1 in $tag is larger than $2 bytes" ;;
        *) cannot "could not download $1 from $tag" ;;
    esac
}

if [ -n "$tag" ]; then
    command -v curl >/dev/null || cannot "curl not found"
    dir=$(mktemp -d) || cannot "could not make a download folder"
    trap 'rm -rf "$dir"' EXIT
    fetch SHA256SUMS 65536
    fetch SHA256SUMS.minisig 4096
fi
[ -f "$dir/SHA256SUMS" ] || fail "no SHA256SUMS in $dir"
[ -f "$dir/SHA256SUMS.minisig" ] || fail "no SHA256SUMS.minisig in $dir"

# -H refuses legacy (unhashed) signatures, as the apps do; minisign accepts them by default.
minisign -V -H -q -p "$key" -m "$dir/SHA256SUMS" -x "$dir/SHA256SUMS.minisig" ||
    fail "SHA256SUMS is not signed with $(basename "$key")"

# Signed, but its names still become paths and URLs: only plain versioned file names pass.
checked=0
while IFS= read -r line; do
    [[ "$line" =~ ^([0-9a-f]{64})\ [\ *](Deskpuck-[0-9]+\.[0-9]+\.[0-9]+-[A-Za-z0-9_.-]+)$ ]] ||
        fail "SHA256SUMS has a line this check does not accept: $line"
    hash=${BASH_REMATCH[1]} name=${BASH_REMATCH[2]}
    [ -z "$tag" ] || fetch "$name" 209715200
    [ -f "$dir/$name" ] || fail "$name is listed but missing"
    [ "$(sha "$dir/$name")" = "$hash" ] || fail "$name does not match SHA256SUMS"
    checked=$((checked + 1))
done <"$dir/SHA256SUMS"
[ $checked -gt 0 ] || fail "SHA256SUMS lists no files"

echo "verify-release: ${tag:-$dir}: signature and $checked file(s) verified"
