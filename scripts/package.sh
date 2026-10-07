#!/bin/bash
# Builds a Linux or Windows release of Deskpuck on that OS, for CI's
# release-artifacts workflow (the Mac's comes from release.sh):
#   scripts/package.sh linux     Deskpuck-<version>-linux-x86_64.tar.gz
#   scripts/package.sh windows   Deskpuck-<version>-windows-x86_64.zip and
#                                Deskpuck-<version>-setup.exe (Inno Setup)
# Each file gets a <file>.sha256 beside it, in dist/. Needs cargo-about, and
# on Windows Inno Setup 6 (ISCC.exe on PATH, or set ISCC).
# Exit 0 done, 1 a step failed, 2 bad usage, 3 a tool is missing.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

fail() { echo "package: $1" >&2; exit 1; }
missing() { echo "package: $1" >&2; exit 3; }

os=${1:-}
case "$os" in
    linux) [ "$(uname -s)" = Linux ] || fail "build the Linux release on Linux" ;;
    windows)
        case "$(uname -s)" in
            MINGW* | MSYS* | CYGWIN*) ;;
            *) fail "build the Windows release on Windows (Git Bash)" ;;
        esac
        ;;
    *) echo "Usage: scripts/package.sh linux|windows" >&2; exit 2 ;;
esac

version=$(sed -n 's/^VERSION="\([0-9][0-9.]*\)"$/\1/p' scripts/make-app.sh)
[ -n "$version" ] || missing "no VERSION in scripts/make-app.sh"
command -v cargo >/dev/null || missing "cargo not found"
command -v cargo-about >/dev/null || missing "cargo-about not found (cargo install --locked --features cli cargo-about)"

name="Deskpuck-$version-$os-x86_64"
out="$PWD/dist"
stage="$out/$name"
rm -rf "$stage" || fail "could not clear $stage"
mkdir -p "$stage" || fail "could not create $stage"

# Release binaries without debug symbols; the Mac build is not affected.
(cd rust && CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --release --locked \
    -p deskpuck-tray -p deskpuck-ble) || fail "cargo build failed"

# Where cargo put them: CARGO_TARGET_DIR or a config can move the target folder.
metadata=$(cd rust && cargo metadata --locked --format-version 1 --no-deps) || fail "cargo metadata failed"
json_path() {
    printf '%s' "$metadata" | grep -o "\"$1\":\"[^\"]*\"" | head -n 1 |
        sed -e "s/^\"$1\":\"//" -e 's/"$//' -e 's/\\\\/\\/g'
}
target_dir=$(json_path target_directory)
[ -n "$target_dir" ] || fail "cargo metadata did not say where the build went"

exe=""
[ "$os" = windows ] && exe=.exe
for program in deskpuck deskpuck-cli; do
    cp "$target_dir/release/$program$exe" "$stage/" || fail "no $program$exe was built"
done

# Licences: Deskpuck's own, then every crate's (cargo-about), then the notice
# files of the fonts the settings window carries, word for word from the
# exact crate version built (the generic licence texts lack their copyrights).
if [ "$os" = windows ]; then
    target=x86_64-pc-windows-msvc
    license=LICENSE.txt
else
    target=x86_64-unknown-linux-gnu
    license=LICENSE
fi
cp LICENSE "$stage/$license" || fail "could not copy LICENSE"
notices="$stage/THIRD-PARTY-NOTICES.txt"
(cd rust && cargo-about generate --locked --fail -m deskpuck-tray/Cargo.toml --target "$target" \
    -o "$notices" about.hbs) || fail "cargo-about failed"
metadata=$(cd rust && cargo metadata --locked --format-version 1) || fail "cargo metadata failed"
fonts_manifest=$(printf '%s' "$metadata" |
    grep -o '"manifest_path":"[^"]*epaint_default_fonts-[^"]*Cargo.toml"' | head -n 1 |
    sed -e 's/^"manifest_path":"//' -e 's/"$//' -e 's/\\\\/\\/g')
[ -n "$fonts_manifest" ] || fail "could not find epaint_default_fonts in cargo metadata"
fonts_dir="$(dirname "$fonts_manifest")/fonts"
(
    printf '\n========================================================================\n'
    printf 'Fonts in the settings window (epaint_default_fonts), as their authors ship them\n'
    printf '========================================================================\n'
    for file in OFL.txt UFL.txt Hack-Regular.txt emoji-icon-font-mit-license.txt; do
        [ -f "$fonts_dir/$file" ] || exit 1
        printf '\n--- %s ---\n\n' "$file"
        cat "$fonts_dir/$file"
    done
) >>"$notices" || fail "a font notice is missing from $fonts_dir"

if [ "$os" = linux ]; then
    cp packaging/linux/deskpuck.desktop packaging/linux/install.sh packaging/linux/README.txt "$stage/" ||
        fail "could not copy the Linux packaging files"
    cp rust/icons/deskpuck-128.png "$stage/deskpuck.png" || fail "could not copy the icon"
    chmod 755 "$stage/deskpuck" "$stage/deskpuck-cli" "$stage/install.sh"
else
    cp packaging/windows/README.txt "$stage/" || fail "could not copy README.txt"
fi

checksum() {
    (cd "$out" && sha256sum "$1" >"$1.sha256") || fail "could not checksum $1"
}

if [ "$os" = linux ]; then
    installer=""
    archive="$name.tar.gz"
    rm -f "$out/$archive"
    tar -C "$out" -czf "$out/$archive" "$name" || fail "could not write $archive"
    checksum "$archive"
else
    archive="$name.zip"
    rm -f "$out/$archive"
    # Windows' own tar (bsdtar) writes the zip; PowerShell 5's Compress-Archive
    # stores backslashes in the paths, which the zip format does not allow.
    "$(cygpath -u "$SYSTEMROOT")/System32/tar.exe" -a -c -f "$(cygpath -w "$out/$archive")" \
        -C "$(cygpath -w "$out")" "$name" || fail "could not write $archive"
    checksum "$archive"

    iscc=${ISCC:-$(command -v ISCC.exe || command -v iscc || true)}
    [ -n "$iscc" ] || missing "Inno Setup's ISCC.exe not found (winget install JRSoftware.InnoSetup, or set ISCC)"
    installer="Deskpuck-$version-setup.exe"
    rm -f "$out/$installer"
    # Git Bash rewrites arguments that start with / into paths; ISCC's switches must pass as typed.
    (cd packaging/windows && MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q "/DVersion=$version" \
        "/DSource=$(cygpath -w "$stage")" "/DOutput=$(cygpath -w "$out")" deskpuck.iss) ||
        fail "Inno Setup failed"
    [ -f "$out/$installer" ] || fail "Inno Setup did not write $installer"
    checksum "$installer"
fi

echo "Built in dist/:"
for file in "$out/$archive" "${installer:+$out/$installer}"; do
    [ -n "$file" ] && printf '  %s (+ .sha256)\n' "$(basename "$file")"
done
exit 0
