#!/bin/sh
# Regenerates these fixtures with two throwaway keys whose secret halves are deleted;
# test.pub is the only key the tests trust. Needs minisign.
set -eu
cd "$(dirname "$0")"
keys=$(mktemp -d)
trap 'rm -rf "$keys"' EXIT
minisign -G -W -f -p test.pub -s "$keys/test.key" >/dev/null
minisign -G -W -f -p "$keys/other.pub" -s "$keys/other.key" >/dev/null

sums() { # version, then the platforms to list
    version=$1
    shift
    for name in "$@"; do
        file="Deskpuck-$version-$name"
        printf '%s %s\n' "$name" "$version" >"assets/$file"
        (cd assets && shasum -a 256 "$file")
    done
}
sign() { minisign -S -s "$keys/${3:-test}.key" -m "$1" -x "$2" -t "Deskpuck test" >/dev/null; }

rm -rf assets older missing duplicate
mkdir -p assets older missing duplicate
all="macos-arm64.zip setup.exe linux-x86_64.tar.gz windows-x86_64.zip"
# shellcheck disable=SC2086
sums 0.4.0 $all >SHA256SUMS
sign SHA256SUMS SHA256SUMS.minisig
sign SHA256SUMS other-key.minisig other
# The other key's signature claiming this key's id: only the maths can refuse it.
python3 -I -c 'import base64, sys
pub = base64.b64decode(open("test.pub").read().split()[-1])
lines = open("other-key.minisig").read().splitlines()
sig = base64.b64decode(lines[1])
lines[1] = base64.b64encode(sig[:2] + pub[2:10] + sig[10:]).decode()
open("forged-id.minisig", "w").write("\n".join(lines) + "\n")'
minisign -S -l -s "$keys/test.key" -m SHA256SUMS -x legacy.minisig -t "Deskpuck test" >/dev/null
# shellcheck disable=SC2086
sums 0.3.0 $all >older/SHA256SUMS
sign older/SHA256SUMS older/SHA256SUMS.minisig
grep -v macos SHA256SUMS >missing/SHA256SUMS
sign missing/SHA256SUMS missing/SHA256SUMS.minisig
{ cat SHA256SUMS; echo "0000000000000000000000000000000000000000000000000000000000000000  Deskpuck-0.4.0-macos-arm64.zip"; } >duplicate/SHA256SUMS
sign duplicate/SHA256SUMS duplicate/SHA256SUMS.minisig
