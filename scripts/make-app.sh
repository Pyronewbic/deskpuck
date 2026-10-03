#!/bin/bash
# Build dist/JoyMouse.app, signed with a persistent identity so macOS keeps
# its Accessibility permission across rebuilds.
#
# Usage: scripts/make-app.sh [--adhoc]
#   --adhoc  sign ad hoc instead (Accessibility must be re-granted after every build)
# Env: JOYMOUSE_SIGN_IDENTITY  signing identity name (default: "JoyMouse Dev")
# Exit: 0 built, 1 build failed, 2 signing identity missing
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="0.1.0"
IDENTITY="${JOYMOUSE_SIGN_IDENTITY:-JoyMouse Dev}"
APP="dist/JoyMouse.app"

adhoc=0
for arg in "$@"; do
    case "$arg" in
        --adhoc) adhoc=1 ;;
        *) echo "Usage: $0 [--adhoc]" >&2; exit 1 ;;
    esac
done

if [ "$adhoc" -eq 1 ]; then
    sign_as="-"
    echo "WARNING: ad hoc signature. macOS will forget JoyMouse's Accessibility permission on every rebuild." >&2
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

swift build -c release --product JoyMouse

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp .build/release/JoyMouse "$APP/Contents/MacOS/JoyMouse"
cp Resources/Info.plist "$APP/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$APP/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$(git rev-list --count HEAD)" "$APP/Contents/Info.plist"
swift scripts/make-icon.swift "$APP/Contents/Resources/AppIcon.icns"

codesign --force --sign "$sign_as" "$APP"
codesign --verify --strict --verbose=1 "$APP"
echo "Built $APP ($VERSION), signed with: ${sign_as/#-/ad hoc}"
