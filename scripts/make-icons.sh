#!/bin/bash
# Renders the Linux and Windows icons from the Mac app's icon, so every
# platform shows the same logo. Run it after changing Resources/Deskpuck.icon
# and commit the PNGs with it:   scripts/make-icons.sh
# Needs Xcode 26's actool, like make-app.sh. Writes rust/icons/deskpuck-N.png.
# Exit 0 done, 1 a step failed, 3 a tool is missing.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 3

fail() { echo "make-icons: $1" >&2; exit 1; }

if ! xcrun --find actool >/dev/null 2>&1; then
    echo "make-icons: actool not found. It needs Xcode 26 (xcode-select -s /Applications/Xcode.app)." >&2
    exit 3
fi

tmp=$(mktemp -d -t deskpuck-icons) || fail "could not make a temporary directory"
trap 'rm -rf "$tmp"' EXIT

xcrun actool Resources/Deskpuck.icon --compile "$tmp" --app-icon Deskpuck \
    --platform macosx --minimum-deployment-target 13.0 --target-device mac \
    --output-partial-info-plist "$tmp/icon.plist" >/dev/null || fail "actool failed"
iconutil -c iconset "$tmp/Deskpuck.icns" -o "$tmp/Deskpuck.iconset" || fail "iconutil failed"

# The largest render is 256 px, laid out on the macOS icon grid: the rounded
# square spans 25..230 and the rest is margin and shadow, which would only
# make the logo smaller in a tray or title bar.
source="$tmp/Deskpuck.iconset/icon_128x128@2x.png"
[ -f "$source" ] || fail "no 256 px render in the icon"
sips --cropOffset 24 24 -c 208 208 "$source" --out "$tmp/logo.png" >/dev/null || fail "could not crop the logo"

mkdir -p rust/icons || fail "could not create rust/icons"
for size in 32 64 128; do
    sips -z "$size" "$size" "$tmp/logo.png" --out "rust/icons/deskpuck-$size.png" >/dev/null ||
        fail "could not write the $size px icon"
done
echo "Wrote rust/icons/deskpuck-{32,64,128}.png"
