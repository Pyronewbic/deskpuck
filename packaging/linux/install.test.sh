#!/bin/bash
# Self-test for install.sh, against a throwaway home whose path has a space
# and a $ in it. Exit 0 when every case behaves, 1 otherwise.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work=$(mktemp -d) || exit 3
trap 'rm -rf "$work"' EXIT
pkg="$work/pkg"
home="$work/home dir \$x"
mkdir -p "$pkg" "$home"
for file in deskpuck deskpuck-cli deskpuck.png; do printf '%s' "$file" >"$pkg/$file"; done
cp "$here/deskpuck.desktop" "$here/install.sh" "$pkg/"

failures=0
check() { if eval "$2"; then :; else echo "FAIL: $1" >&2; failures=$((failures + 1)); fi; }
run() { XDG_BIN_HOME="$home/bin" XDG_DATA_HOME="$home/share" sh "$pkg/install.sh" "$@" >/dev/null 2>&1; }

desktop="$home/share/applications/deskpuck.desktop"
icon="$home/share/icons/hicolor/128x128/apps/deskpuck.png"

run; status=$?
check "install exits 0" '[ $status -eq 0 ]'
check "programs installed and executable" '[ -x "$home/bin/deskpuck" ] && [ -x "$home/bin/deskpuck-cli" ]'
check "icon installed" '[ "$(cat "$icon")" = deskpuck.png ]'
# Desktop entries escape $ inside a quoted argument with a backslash.
expected="Exec=\"$work/home dir \\\$x/bin/deskpuck\""
check "menu entry runs the installed program, quoted and escaped" \
    '[ "$(grep ^Exec= "$desktop")" = "$expected" ]'
check "the rest of the entry is unchanged" \
    '[ "$(grep -v ^Exec= "$desktop")" = "$(grep -v ^Exec= "$pkg/deskpuck.desktop")" ]'

run; status=$?
check "a second install exits 0" '[ $status -eq 0 ]'
check "no temporary file is left" '[ -z "$(ls "$home/share/applications" | grep tmp)" ]'

run --nope; status=$?
check "a bad argument exits 2" '[ $status -eq 2 ]'

rm "$pkg/deskpuck.png"
rm -f "$home/bin/deskpuck"
run; status=$?
check "a missing file exits 1" '[ $status -eq 1 ]'
check "a missing file installs nothing" '[ ! -e "$home/bin/deskpuck" ]'

run --uninstall; status=$?
check "uninstall exits 0" '[ $status -eq 0 ]'
check "uninstall removes what was installed" \
    '[ ! -e "$home/bin/deskpuck-cli" ] && [ ! -e "$desktop" ] && [ ! -e "$icon" ]'

if [ $failures -ne 0 ]; then
    exit 1
fi
echo "install.sh: all cases behave"
