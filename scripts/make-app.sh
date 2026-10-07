#!/bin/bash
# Build dist/Deskpuck.app, signed with a persistent identity so macOS keeps
# its Accessibility permission across rebuilds.
#
# Usage: scripts/make-app.sh [--adhoc]
#   --adhoc  sign ad hoc instead (Accessibility must be re-granted after every build)
# Env: DESKPUCK_SIGN_IDENTITY  signing identity name (default: "Deskpuck Dev")
# Exit: 0 built, 1 build failed, 2 signing identity missing
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="0.3.0"
IDENTITY="${DESKPUCK_SIGN_IDENTITY:-Deskpuck Dev}"
APP="dist/Deskpuck.app"

adhoc=0
for arg in "$@"; do
    case "$arg" in
        --adhoc) adhoc=1 ;;
        *) echo "Usage: $0 [--adhoc]" >&2; exit 1 ;;
    esac
done

if [ "$adhoc" -eq 1 ]; then
    sign_as="-"
    echo "WARNING: ad hoc signature. macOS will forget Deskpuck's Accessibility permission on every rebuild." >&2
else
    # No -v: a self-signed identity is "not trusted", which -v would hide, but codesign accepts it.
    identities=$(security find-identity -p codesigning)
    if ! printf '%s\n' "$identities" | grep -qF "\"$IDENTITY\""; then
        cat >&2 <<EOF
No code signing identity named "$IDENTITY" was found.

Create it once (it stays in your login keychain):
  1. Open Keychain Access.
  2. Choose Keychain Access > Certificate Assistant > Create a Certificate...
  3. Name: $IDENTITY   Identity Type: Self Signed Root   Certificate Type: Code Signing
  4. Click Create, then Done.

Then run this script again. To build without it, use --adhoc.
EOF
        exit 2
    fi
    sign_as="$IDENTITY"
fi

if ! xcrun --find actool >/dev/null 2>&1; then
    echo "actool not found. The app icon needs Xcode 26 (xcode-select -s /Applications/Xcode.app)." >&2
    exit 1
fi

# Any Rust build failure is a build failure (1); 2 means a missing signing identity.
scripts/build-rust.sh || exit 1
# SwiftPM does not see the Rust library as an input, so after a Rust-only
# change it would keep a binary linked against the old library. Removing the
# binary forces the link; the check below proves it happened.
rust_lib=rust/target/release/libdeskpuck_ffi.a
rm -f .build/release/Deskpuck
swift build -c release --product Deskpuck || exit 1
if [ ! .build/release/Deskpuck -nt "$rust_lib" ]; then
    echo "the app was not linked after $rust_lib was built; it would ship an old Rust core" >&2
    exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp .build/release/Deskpuck "$APP/Contents/MacOS/Deskpuck"
cp Resources/Info.plist "$APP/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$APP/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$(git rev-list --count HEAD)" "$APP/Contents/Info.plist"
# Resources/Deskpuck.icon is the layered macOS 26 icon; actool also derives a .icns for older macOS.
icon_plist=$(mktemp -t deskpuck-icon)
xcrun actool Resources/Deskpuck.icon --compile "$APP/Contents/Resources" --app-icon Deskpuck \
    --platform macosx --minimum-deployment-target 13.0 --target-device mac \
    --output-partial-info-plist "$icon_plist" >/dev/null
for key in CFBundleIconFile CFBundleIconName; do
    plutil -replace "$key" -string "$(plutil -extract "$key" raw "$icon_plist")" "$APP/Contents/Info.plist"
done
rm -f "$icon_plist"

# Hardened runtime: without it, DYLD_INSERT_LIBRARIES can load code into the app
# and borrow its Accessibility permission.
codesign --force --options runtime --sign "$sign_as" "$APP"
codesign --verify --strict --verbose=1 "$APP"
signature=$(codesign -dv "$APP" 2>&1)
if ! grep -qE '^CodeDirectory .*flags=0x[0-9a-f]*\([^)]*runtime' <<<"$signature"; then
    echo "$APP is not signed with the hardened runtime" >&2
    exit 1
fi
echo "Built $APP ($VERSION), signed with: ${sign_as/#-/ad hoc}"
