#!/bin/bash
# Package dist/Deskpuck.app (from scripts/make-app.sh) as a disk image and a zip.
# Usage: scripts/make-dmg.sh
set -euo pipefail

cd "$(dirname "$0")/.."

APP="dist/Deskpuck.app"
DMGBUILD_VERSION="1.6.7"

if [ ! -d "$APP" ]; then
    echo "$APP not found; run scripts/make-app.sh first." >&2
    exit 1
fi
codesign --verify --strict "$APP"

version=$(plutil -extract CFBundleShortVersionString raw "$APP/Contents/Info.plist")
case "$(lipo -archs "$APP/Contents/MacOS/Deskpuck")" in
    arm64) arch=arm64 ;;
    x86_64) arch=x86_64 ;;
    *) arch=universal ;;
esac
dmg="dist/Deskpuck-$version-macos-$arch.dmg"
zip="dist/Deskpuck-$version-macos-$arch.zip"

if [ ! -x .venv/bin/dmgbuild ] || ! .venv/bin/pip show dmgbuild 2>/dev/null | grep -qx "Version: $DMGBUILD_VERSION"; then
    # --clear: a venv records absolute paths, so one from a moved checkout is broken.
    python3 -m venv --clear .venv
    .venv/bin/pip install --quiet "dmgbuild==$DMGBUILD_VERSION"
fi

rm -f "$dmg" "$zip"
.venv/bin/dmgbuild -s scripts/dmg_settings.py -D app="$APP" "Deskpuck" "$dmg"
hdiutil verify "$dmg"
ditto -c -k --keepParent "$APP" "$zip"
echo "Built $dmg and $zip"
