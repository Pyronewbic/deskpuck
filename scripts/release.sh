#!/bin/bash
# Usage: scripts/release.sh [--artifacts | --publish]; signing stays on this Mac. --artifacts and --publish need GH_TOKEN.
# Exit 0 done, 1 a check failed, 2 bad usage, 3 a check could not run.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

fail() { echo "release: $1" >&2; exit 1; }
cannot() { echo "release: CANNOT CHECK: $1" >&2; exit 3; }

mode=build
case "${1:-}" in
    "") ;;
    --artifacts) mode=artifacts ;;
    --publish) mode=publish ;;
    *) echo "Usage: scripts/release.sh [--artifacts | --publish]" >&2; exit 2 ;;
esac

version=$(sed -n 's/^VERSION="\([0-9][0-9.]*\)"$/\1/p' scripts/make-app.sh)
[ -n "$version" ] || cannot "no VERSION in scripts/make-app.sh"
tag="v$version"
head=$(git rev-parse HEAD) || cannot "not a git checkout"

github_repo() {
    [ -n "${GH_TOKEN:-}" ] || fail "--$mode needs GH_TOKEN for the account that owns the release"
    # From origin, never gh's default: with an upstream remote, gh picks the fork's parent.
    repo=$(git remote get-url origin | sed -E 's#^(https://github\.com/|git@github\.com:)##; s#\.git$##')
    [[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || cannot "origin is not a GitHub repo: $repo"
    [ "$(gh api "repos/$repo" --jq .permissions.push)" = true ] || fail "GH_TOKEN cannot push to $repo"
}

ci_assets=(
    "Deskpuck-$version-linux-x86_64.tar.gz"
    "Deskpuck-$version-windows-x86_64.zip"
    "Deskpuck-$version-setup.exe"
)

fetch_ci_assets() {
    local run info file found
    run=$(gh run list --repo "$repo" --workflow release-artifacts.yml --commit "$head" --status success \
        --limit 1 --json databaseId --jq '.[0].databaseId // empty') || cannot "could not list workflow runs"
    if [ -z "$run" ]; then
        fail "no passing release-artifacts run for $(git rev-parse --short HEAD). Start one with
  gh workflow run release-artifacts.yml --repo $repo --ref main
then run this again once it has passed."
    fi
    # Who built them, not only that they exist: this repo's workflow, this commit, passed.
    info=$(gh api "repos/$repo/actions/runs/$run" \
        --jq '[.path, .head_sha, .conclusion, .repository.full_name] | join(" ")') || cannot "could not read run $run"
    [ "$info" = ".github/workflows/release-artifacts.yml $head success $repo" ] ||
        fail "run $run is not a passing release-artifacts run of $repo for this commit: $info"

    rm -rf dist/ci || fail "could not clear dist/ci"
    gh run download "$run" --repo "$repo" -n deskpuck-linux -n deskpuck-windows -D dist/ci ||
        fail "could not download the artifacts of run $run"
    for file in "${ci_assets[@]}"; do
        found=$(find dist/ci -type f -name "$file")
        [ "$(printf '%s\n' "$found" | grep -c .)" -eq 1 ] || fail "run $run has no single $file"
        [ -f "$found.sha256" ] || fail "run $run has no $file.sha256"
        (cd "$(dirname "$found")" && shasum -a 256 -c "$file.sha256") || fail "$file does not match its .sha256"
        gh attestation verify "$found" --repo "$repo" --source-digest "$head" --deny-self-hosted-runners \
            --signer-workflow "$repo/.github/workflows/release-artifacts.yml" >/dev/null ||
            fail "$file has no attestation from release-artifacts.yml in $repo for this commit"
        cp "$found" "dist/$file" || fail "could not copy $file to dist/"
    done
    echo "Fetched the Linux and Windows builds of run $run and verified their checksums and attestations."
}

if [ "$mode" = artifacts ]; then
    github_repo
    fetch_ci_assets
    exit 0
fi

[ -z "$(git status --porcelain)" ] || fail "the working tree has uncommitted changes"
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "not on main"
git fetch --quiet --tags origin || cannot "could not fetch origin"
[ "$head" = "$(git rev-parse origin/main)" ] || fail "main is not the same as origin/main"
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    fail "$tag already exists"
fi
notes=$(awk -v v="$version" '$0 ~ "^## \\[" v "\\]" { on = 1; next } /^## \[/ { on = 0 } on' CHANGELOG.md)
[ -n "$(tr -d '[:space:]' <<<"$notes")" ] || fail "CHANGELOG.md has no notes under ## [$version]"

# Before the long build, so a missing CI run fails fast and nothing is tagged.
assets=("Deskpuck-$version.dmg" "Deskpuck-$version.zip")
if [ "$mode" = publish ]; then
    github_repo
    fetch_ci_assets
    assets+=("${ci_assets[@]}")
fi

scripts/check.sh
status=$?
[ $status -eq 0 ] || exit $status
scripts/make-app.sh || fail "make-app.sh failed"
signature=$(codesign -dvv dist/Deskpuck.app 2>&1)
grep -qx "Authority=${DESKPUCK_SIGN_IDENTITY:-Deskpuck Dev}" <<<"$signature" ||
    fail "dist/Deskpuck.app is not signed with ${DESKPUCK_SIGN_IDENTITY:-Deskpuck Dev}; users would lose Accessibility on update"
scripts/make-dmg.sh || fail "make-dmg.sh failed"

(cd dist && shasum -a 256 "${assets[@]}" > SHA256SUMS) || fail "could not write dist/SHA256SUMS"
(cd dist && shasum -a 256 -c SHA256SUMS) || fail "dist/SHA256SUMS does not verify"

if [ "$mode" = build ]; then
    echo "Built $tag from $(git rev-parse --short HEAD). To publish: scripts/release.sh --publish"
    exit 0
fi

git tag -s "$tag" -m "Deskpuck $version" || fail "could not create the signed tag $tag"
# shellcheck disable=SC2016 # git's shell expands GH_TOKEN, keeping it off the command line
git -c credential.helper= \
    -c 'credential.helper=!f() { echo username=x-access-token; echo "password=$GH_TOKEN"; }; f' \
    push origin "refs/tags/$tag" || fail "could not push $tag"
uploads=()
for asset in "${assets[@]}"; do
    uploads+=("dist/$asset")
done
gh release create "$tag" --repo "$repo" --draft --verify-tag --title "Deskpuck $version" --notes "$notes" \
    "${uploads[@]}" dist/SHA256SUMS || fail "could not create the draft release"
echo "Draft release $tag created. Review it on GitHub, then publish it there."
