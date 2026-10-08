#!/bin/bash
# Self-test for aur-update.sh, in a throwaway repo with stub gh, makepkg and git fetch. Exit 0 when every case behaves, 1 otherwise.
# Checks are single-quoted conditions eval'd by check().
# shellcheck disable=SC2016,SC2034
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d) || exit 3
trap 'rm -rf "$work"' EXIT
repo="$work/repo"
aur="$repo/packaging/aur/deskpuck-bin"
mkdir -p "$repo/scripts" "$aur" "$work/bin"
cp "$root/scripts/aur-update.sh" "$repo/scripts/"
cp "$root/packaging/aur/deskpuck-bin/PKGBUILD" "$root/packaging/aur/deskpuck-bin/deskpuck.install" \
    "$root/packaging/aur/deskpuck-bin/60-deskpuck-uinput.rules" "$aur/" || exit 3
echo 'VERSION="9.8.7"' >"$repo/scripts/make-app.sh"
git -C "$repo" init -q && git -C "$repo" remote add origin https://github.com/owner/proj.git &&
    git -C "$repo" -c user.name=t -c user.email=t@t commit -q --allow-empty -m init &&
    git -C "$repo" tag v9.8.7 || exit 3
commit=$(git -C "$repo" rev-parse HEAD)
rules_sum=$(sed -n "s/^ *'\([0-9a-f]\{64\}\)')\$/\1/p" "$aur/PKGBUILD")
[ -n "$rules_sum" ] || exit 3
# An earlier pkgrel, so restarting it at 1 is visible.
sed 's/^pkgrel=.*/pkgrel=3/' "$aur/PKGBUILD" >"$work/PKGBUILD.orig" || exit 3

real_git=$(command -v git)
cat >"$work/bin/git" <<STUB
#!/bin/bash
[ "\$1" = fetch ] && exit "\${STUB_FETCH:-0}"
exec "$real_git" "\$@"
STUB

# The release holds a tarball whose bytes are its own name; STUB_* pick what goes wrong.
cat >"$work/bin/gh" <<'STUB'
#!/bin/bash
case "$1 $2" in
    "release view") echo "${STUB_DRAFT:-false}" ;;
    "release download")
        [ -z "${STUB_DOWNLOAD:-}" ] || exit 1
        prev="" dir=""
        for arg; do [ "$prev" = -D ] && dir=$arg; prev=$arg; done
        f=Deskpuck-9.8.7-linux-x86_64.tar.gz
        printf '%s' "$f" >"$dir/$f"
        if command -v sha256sum >/dev/null; then hash=sha256sum; else hash="shasum -a 256"; fi
        (cd "$dir" && $hash "$f" >SHA256SUMS)
        if [ -n "${STUB_CORRUPT:-}" ]; then printf x >>"$dir/$f"; fi ;;
    "attestation verify") echo "$*" >>"$STUB_LOG"; exit "${STUB_ATTEST:-0}" ;;
    *) echo "stub gh: unexpected $*" >&2; exit 99 ;;
esac
STUB

# Prints the two .SRCINFO lines the script checks, read from the PKGBUILD it is given.
cat >"$work/bin/makepkg" <<'STUB'
#!/bin/bash
[ "$1" = --printsrcinfo ] || exit 99
[ -z "${STUB_MAKEPKG:-}" ] || exit 1
printf 'pkgbase = deskpuck-bin\n'
printf '\tpkgver = %s\n' "${STUB_SRCINFO_VER:-$(sed -n 's/^pkgver=//p' PKGBUILD)}"
sed -n "s/^sha256sums=('\([0-9a-f]*\)'\$/\tsha256sums = \1/p" PKGBUILD
STUB
chmod +x "$work/bin/git" "$work/bin/gh" "$work/bin/makepkg"

f=Deskpuck-9.8.7-linux-x86_64.tar.gz
if command -v sha256sum >/dev/null; then hash=sha256sum; else hash="shasum -a 256"; fi
want_sum=$(printf '%s' "$f" | $hash | cut -d' ' -f1)

failures=0
check() {
    if ! eval "$2"; then
        echo "FAIL: $1" >&2
        sed 's/^/    /' "$work/out" >&2
        failures=$((failures + 1))
    fi
}
reset() {
    cp "$work/PKGBUILD.orig" "$aur/PKGBUILD"
    rm -f "$aur/.SRCINFO" "$work/log"
}
run() {
    (cd "$repo" && PATH="$work/bin:$PATH" STUB_LOG="$work/log" "$@" scripts/aur-update.sh) >"$work/out" 2>&1
}
unchanged() { cmp -s "$work/PKGBUILD.orig" "$aur/PKGBUILD" && [ ! -e "$aur/.SRCINFO" ]; }
no_leftovers() { [ -z "$(find "$aur" -name '.*.??????' 2>/dev/null)" ]; }
field() { sed -n "s/^$1=//p" "$aur/PKGBUILD"; }

reset; run env; status=$?
check "a verified release exits 0" '[ $status -eq 0 ]'
check "pkgver is the new version" '[ "$(field pkgver)" = 9.8.7 ]'
check "pkgrel restarts at 1" '[ "$(field pkgrel)" = 1 ]'
check "the tarball checksum is pinned" 'grep -qF "sha256sums=('"'"'$want_sum'"'"'" "$aur/PKGBUILD"'
check "the udev rule checksum is kept" 'grep -qF "'"'"'$rules_sum'"'"')" "$aur/PKGBUILD"'
check ".SRCINFO is written from the new PKGBUILD" \
    'grep -qxF "$(printf "\tpkgver = 9.8.7")" "$aur/.SRCINFO"'
want="--repo owner/proj --source-digest $commit --deny-self-hosted-runners"
want="$want --signer-workflow owner/proj/.github/workflows/release-artifacts.yml"
check "the attestation must come from this repo's workflow at the tag's commit" \
    '[ "$(grep -cF -- "$want" "$work/log")" -eq 1 ]'
check "no temporary files are left" 'no_leftovers'

cp "$aur/PKGBUILD" "$work/PKGBUILD.done"; rm -f "$aur/.SRCINFO"; run env; status=$?
check "a second run exits 0" '[ $status -eq 0 ]'
check "and leaves the PKGBUILD as it was" 'cmp -s "$work/PKGBUILD.done" "$aur/PKGBUILD"'
check "but regenerates .SRCINFO" '[ -s "$aur/.SRCINFO" ]'

sed "s/^sha256sums=('[0-9a-f]*'/sha256sums=('$(printf '%064d' 0)'/" "$work/PKGBUILD.done" >"$work/PKGBUILD.orig"
reset; run env; status=$?
check "the same version with another checksum exits 1" '[ $status -eq 1 ]'
check "it says so" 'grep -q "already pins .* with another checksum" "$work/out"'
check "and changes nothing" 'unchanged && no_leftovers'
sed 's/^pkgrel=.*/pkgrel=3/' "$root/packaging/aur/deskpuck-bin/PKGBUILD" >"$work/PKGBUILD.orig"

reset; run env STUB_DRAFT=true; status=$?
check "a draft release exits 1" '[ $status -eq 1 ] && unchanged'
check "before anything is downloaded" '[ ! -e "$work/log" ]'

reset; run env STUB_ATTEST=1; status=$?
check "a tarball without a valid attestation exits 1" '[ $status -eq 1 ] && unchanged && no_leftovers'
check "it says so" 'grep -q "has no attestation from release-artifacts.yml" "$work/out"'

reset; run env STUB_CORRUPT=1; status=$?
check "a tarball that does not match SHA256SUMS exits 1" '[ $status -eq 1 ] && unchanged'
check "before its attestation is checked" '[ ! -e "$work/log" ]'

reset; run env STUB_DOWNLOAD=1; status=$?
check "a failed download is could-not-check (3)" '[ $status -eq 3 ] && unchanged'

reset; run env STUB_MAKEPKG=1; status=$?
check "a failed makepkg is could-not-check (3)" '[ $status -eq 3 ] && unchanged && no_leftovers'

reset; run env STUB_SRCINFO_VER=0.0.0; status=$?
check "a .SRCINFO that does not match exits 1" '[ $status -eq 1 ] && unchanged && no_leftovers'

reset; git -C "$repo" tag -d v9.8.7 >/dev/null; run env; status=$?
check "a missing tag exits 1" '[ $status -eq 1 ] && unchanged'
git -C "$repo" tag v9.8.7

reset; run env STUB_FETCH=1; status=$?
check "an unreachable origin is could-not-check (3)" '[ $status -eq 3 ] && unchanged'

if [ $failures -ne 0 ]; then
    exit 1
fi
echo "aur-update.sh: all cases behave"
