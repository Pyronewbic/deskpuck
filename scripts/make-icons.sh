#!/bin/bash
# Renders rust/icons from the Mac icon; run after changing Resources/Deskpuck.icon, commit the output.
# Exit 0 done, 1 a step failed, 3 a tool is missing (needs Xcode 26 actool).
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

# Crop to the rounded square (25..230 of 256 px); the margin and shadow would shrink the logo.
source="$tmp/Deskpuck.iconset/icon_128x128@2x.png"
[ -f "$source" ] || fail "no 256 px render in the icon"
sips --cropOffset 24 24 -c 208 208 "$source" --out "$tmp/logo.png" >/dev/null || fail "could not crop the logo"

mkdir -p rust/icons || fail "could not create rust/icons"
for size in 32 64 128; do
    sips -z "$size" "$size" "$tmp/logo.png" --out "rust/icons/deskpuck-$size.png" >/dev/null ||
        fail "could not write the $size px icon"
done
ico_sizes="16 24 32 48 64 128 256"
for size in $ico_sizes; do
    sips -z "$size" "$size" "$tmp/logo.png" --out "$tmp/ico-$size.png" >/dev/null ||
        fail "could not make the $size px .ico entry"
done
python3 - "$tmp" rust/icons/deskpuck.ico $ico_sizes <<'PY' || fail "could not write deskpuck.ico"
import struct, sys
tmp, out, sizes = sys.argv[1], sys.argv[2], [int(s) for s in sys.argv[3:]]
images = [open(f"{tmp}/ico-{size}.png", "rb").read() for size in sizes]
header = struct.pack("<HHH", 0, 1, len(images))
offset = len(header) + 16 * len(images)
entries = b""
for size, image in zip(sizes, images):
    # Width and height 0 mean 256; 32 bits per pixel; then the PNG's length and offset.
    entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(image), offset)
    offset += len(image)
with open(out, "wb") as f:
    f.write(header + entries + b"".join(images))
PY
echo "Wrote rust/icons/deskpuck-{32,64,128}.png and rust/icons/deskpuck.ico"
