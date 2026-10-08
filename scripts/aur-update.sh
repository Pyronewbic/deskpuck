#!/bin/bash
# Usage: scripts/aur-update.sh; points packaging/aur/deskpuck-bin at the published release of VERSION.
# Exit 0 done, 1 a check failed, 2 bad usage, 3 a check could not run.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

fail() { echo "aur-update: $1" >&2; exit 1; }
cannot() { echo "aur-update: CANNOT CHECK: $1" >&2; exit 3; }

[ $# -eq 0 ] || { echo "Usage: scripts/aur-update.sh" >&2; exit 2; }

version=$(sed -n 's/^VERSION="\([0-9][0-9.]*\)"$/\1/p' scripts/make-app.sh)
[ -n "$version" ] || cannot "no VERSION in scripts/make-app.sh"
tag="v$version"
dir=packaging/aur/deskpuck-bin
tarball="Deskpuck-$version-linux-x86_64.tar.gz"
[ -f "$dir/PKGBUILD" ] || cannot "no $dir/PKGBUILD"

if command -v sha256sum >/dev/null; then
    sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
    sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

repo=$(git remote get-url origin | sed -E 's#^(https://github\.com/|git@github\.com:)##; s#\.git$##')
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || cannot "origin is not a GitHub repo: $repo"
git fetch --quiet --tags origin || cannot "could not fetch origin"
commit=$(git rev-parse -q --verify "refs/tags/$tag^{commit}") || fail "no tag $tag; publish the release first"

# AUR users download from the public URL, which a draft does not have yet.
draft=$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft) || cannot "could not read release $tag"
[ "$draft" = false ] || fail "release $tag is still a draft; publish it on GitHub first"

work=$(mktemp -d) || cannot "mktemp failed"
trap 'rm -rf "$work"' EXIT
gh release download "$tag" --repo "$repo" -p "$tarball" -p SHA256SUMS -D "$work" ||
    cannot "could not download $tarball from release $tag"
sum=$(sha256 "$work/$tarball")
listed=$(awk -v f="$tarball" '$2 == f { print $1 }' "$work/SHA256SUMS")
[ -n "$listed" ] || fail "SHA256SUMS of $tag does not list $tarball"
[ "$sum" = "$listed" ] || fail "$tarball does not match SHA256SUMS of $tag"
gh attestation verify "$work/$tarball" --repo "$repo" --source-digest "$commit" --deny-self-hosted-runners \
    --signer-workflow "$repo/.github/workflows/release-artifacts.yml" >/dev/null ||
    fail "$tarball has no attestation from release-artifacts.yml in $repo for $tag"

current=$(sed -n 's/^pkgver=//p' "$dir/PKGBUILD")
pinned=$(sed -n "s/^sha256sums=('\([0-9a-f]\{64\}\)'\$/\1/p" "$dir/PKGBUILD")
[ -n "$current" ] && [ -n "$pinned" ] || cannot "$dir/PKGBUILD has no pkgver= or sha256sums=('<hex>' line"
if [ "$current" = "$version" ] && [ "$pinned" != "$sum" ]; then
    fail "PKGBUILD already pins $tarball with another checksum; the release changed after it was packaged"
fi

# mktemp+mv beside the target, so a symlink in its place is replaced, not followed.
new=$(mktemp "$dir/.PKGBUILD.XXXXXX") || cannot "mktemp failed"
if [ "$current" = "$version" ]; then
    cp "$dir/PKGBUILD" "$new"
else
    sed -e "s/^pkgver=.*/pkgver=$version/" -e 's/^pkgrel=.*/pkgrel=1/' \
        -e "s/^sha256sums=('[0-9a-f]\{64\}'\$/sha256sums=('$sum'/" "$dir/PKGBUILD" >"$new"
fi || { rm -f "$new"; cannot "could not write PKGBUILD"; }

srcinfo=$(mktemp "$dir/.SRCINFO.XXXXXX") || { rm -f "$new"; cannot "mktemp failed"; }
cleanup_new() { rm -f "$new" "$srcinfo"; }
if command -v makepkg >/dev/null; then
    stage="$work/pkg"
    mkdir "$stage" && cp "$new" "$stage/PKGBUILD" && cp "$dir/deskpuck.install" "$dir/60-deskpuck-uinput.rules" "$stage/" &&
        (cd "$stage" && makepkg --printsrcinfo) >"$srcinfo"
elif command -v docker >/dev/null; then
    # The Arch image is x86-64 only.
    docker run --rm -i --platform linux/amd64 archlinux:latest bash -c \
        'useradd -m b && mkdir /p && cat >/p/PKGBUILD && touch /p/deskpuck.install && chown -R b /p &&
         cd /p && su b -c "makepkg --printsrcinfo"' <"$new" >"$srcinfo"
else
    cleanup_new
    cannot "makepkg or docker is needed to write .SRCINFO"
fi || { cleanup_new; cannot ".SRCINFO could not be generated"; }

if ! { grep -qxF $'\tpkgver = '"$version" "$srcinfo" && grep -qxF $'\tsha256sums = '"$sum" "$srcinfo"; }; then
    cleanup_new
    fail "the new .SRCINFO does not name $version and its checksum"
fi
if ! { mv -f "$new" "$dir/PKGBUILD" && mv -f "$srcinfo" "$dir/.SRCINFO"; }; then
    cleanup_new
    cannot "could not replace the files"
fi

if [ "$current" = "$version" ]; then
    echo "$dir already pins $tag; regenerated .SRCINFO."
else
    echo "Pointed $dir at $tag ($sum), verified against SHA256SUMS and its attestation."
fi
