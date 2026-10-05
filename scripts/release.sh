#!/bin/bash
# Builds a release on this Mac, so the signing key never leaves it.
#   scripts/release.sh            check, build, and write dist/SHA256SUMS
#   scripts/release.sh --publish  then push a signed tag and create a draft
#                                 GitHub release, published by hand after review
# For --publish, GH_TOKEN must belong to an account that can push to the repo.
# Exit 0 done, 1 a check failed, 2 bad usage, 3 a check could not run.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

fail() { echo "release: $1" >&2; exit 1; }
cannot() { echo "release: CANNOT CHECK: $1" >&2; exit 3; }

publish=false
case "${1:-}" in
    "") ;;
    --publish) publish=true ;;
    *) echo "Usage: scripts/release.sh [--publish]" >&2; exit 2 ;;
esac

version=$(sed -n 's/^VERSION="\([0-9][0-9.]*\)"$/\1/p' scripts/make-app.sh)
[ -n "$version" ] || cannot "no VERSION in scripts/make-app.sh"
tag="v$version"

[ -z "$(git status --porcelain)" ] || fail "the working tree has uncommitted changes"
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "not on main"
git fetch --quiet --tags origin || cannot "could not fetch origin"
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || fail "main is not the same as origin/main"
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    fail "$tag already exists"
fi
notes=$(awk -v v="$version" '$0 ~ "^## \\[" v "\\]" { on = 1; next } /^## \[/ { on = 0 } on' CHANGELOG.md)
[ -n "$(tr -d '[:space:]' <<<"$notes")" ] || fail "CHANGELOG.md has no notes under ## [$version]"

scripts/check.sh
status=$?
[ $status -eq 0 ] || exit $status
scripts/make-app.sh || fail "make-app.sh failed"
signature=$(codesign -dvv dist/Deskpuck.app 2>&1)
grep -qx "Authority=${DESKPUCK_SIGN_IDENTITY:-Deskpuck Dev}" <<<"$signature" ||
    fail "dist/Deskpuck.app is not signed with ${DESKPUCK_SIGN_IDENTITY:-Deskpuck Dev}; users would lose Accessibility on update"
scripts/make-dmg.sh || fail "make-dmg.sh failed"

assets=("Deskpuck-$version.dmg" "Deskpuck-$version.zip")
(cd dist && shasum -a 256 "${assets[@]}" > SHA256SUMS) || fail "could not write dist/SHA256SUMS"
(cd dist && shasum -a 256 -c SHA256SUMS) || fail "dist/SHA256SUMS does not verify"

if ! $publish; then
    echo "Built $tag from $(git rev-parse --short HEAD). To publish: scripts/release.sh --publish"
    exit 0
fi

[ -n "${GH_TOKEN:-}" ] || fail "--publish needs GH_TOKEN for the account that owns the release"
# From origin, never gh's default: with an upstream remote, gh picks the fork's parent.
repo=$(git remote get-url origin | sed -E 's#^(https://github\.com/|git@github\.com:)##; s#\.git$##')
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || cannot "origin is not a GitHub repo: $repo"
[ "$(gh api "repos/$repo" --jq .permissions.push)" = true ] || fail "GH_TOKEN cannot push to $repo"

git tag -s "$tag" -m "Deskpuck $version" || fail "could not create the signed tag $tag"
# shellcheck disable=SC2016 # git's shell expands GH_TOKEN, keeping it off the command line
git -c credential.helper= \
    -c 'credential.helper=!f() { echo username=x-access-token; echo "password=$GH_TOKEN"; }; f' \
    push origin "refs/tags/$tag" || fail "could not push $tag"
gh release create "$tag" --repo "$repo" --draft --verify-tag --title "Deskpuck $version" --notes "$notes" \
    "dist/${assets[0]}" "dist/${assets[1]}" dist/SHA256SUMS || fail "could not create the draft release"
echo "Draft release $tag created. Review it on GitHub, then publish it there."
