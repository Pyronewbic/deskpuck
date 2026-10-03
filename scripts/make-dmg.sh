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
dmg="dist/Deskpuck-$version.dmg"
zip="dist/Deskpuck-$version.zip"

if [ ! -x .venv/bin/dmgbuild ] || ! .venv/bin/pip show dmgbuild 2>/dev/null | grep -qx "Version: $DMGBUILD_VERSION"; then
    python3 -m venv .venv
    .venv/bin/pip install --quiet "dmgbuild==$DMGBUILD_VERSION"
fi

rm -f "$dmg" "$zip"
.venv/bin/dmgbuild -s scripts/dmg_settings.py -D app="$APP" "Deskpuck" "$dmg"
hdiutil verify "$dmg"
ditto -c -k --keepParent "$APP" "$zip"
echo "Built $dmg and $zip"
