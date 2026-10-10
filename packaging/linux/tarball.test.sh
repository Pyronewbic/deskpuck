#!/bin/bash
# Usage: packaging/linux/tarball.test.sh TARBALL (Linux, dbus-daemon, cargo): installs, runs, reinstalls over the running tray, uninstalls.
# Exit 0 every case behaves, 1 a case failed, 2 bad usage, 3 could not run.
set -uo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)" || exit 3
fail() { echo "FAIL: $1" >&2; exit 1; }
cannot() { echo "tarball.test.sh: CANNOT CHECK: $1" >&2; exit 3; }

[ $# -eq 1 ] && [ -f "$1" ] || { echo "Usage: packaging/linux/tarball.test.sh TARBALL" >&2; exit 2; }
[ "$(uname -s)" = Linux ] || cannot "runs on Linux"
for tool in dbus-daemon cargo tar; do
    command -v "$tool" >/dev/null || cannot "$tool not found"
done
tarball="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"

work=$(mktemp -d) || cannot "no temporary folder"
tray="" bus=""
cleanup() {
    [ -z "$tray" ] || kill "$tray" 2>/dev/null
    [ -z "$bus" ] || kill "$bus" 2>/dev/null
    rm -rf "$work"
}
trap cleanup EXIT
# cargo finds its toolchains through HOME, which is about to become a scratch folder.
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export XDG_BIN_HOME="$work/bin" XDG_DATA_HOME="$work/data" HOME="$work/home"
mkdir -p "$HOME"

tar -xzf "$tarball" -C "$work" || fail "the tarball does not unpack"
folders=("$work"/Deskpuck-*-linux-x86_64)
if [ ${#folders[@]} -ne 1 ] || [ ! -d "${folders[0]}" ]; then
    fail "want one Deskpuck-*-linux-x86_64 folder"
fi
folder=${folders[0]}

"$folder/install.sh" >/dev/null || fail "install.sh failed"
for program in deskpuck deskpuck-cli; do
    [ -x "$XDG_BIN_HOME/$program" ] || fail "$program is not installed"
    cmp -s "$folder/$program" "$XDG_BIN_HOME/$program" || fail "the installed $program differs from the tarball's"
done
grep -qx "Exec=\"$XDG_BIN_HOME/deskpuck\"" "$XDG_DATA_HOME/applications/deskpuck.desktop" ||
    fail "the menu entry does not start the installed copy"
[ -f "$XDG_DATA_HOME/icons/hicolor/128x128/apps/deskpuck.png" ] || fail "the icon is not installed"

# The installed tray, through the test that otherwise runs cargo's own build.
(cd "$root/rust" && DESKPUCK_TRAY_BIN="$XDG_BIN_HOME/deskpuck" \
    cargo test --locked --quiet -p deskpuck-tray --test tray_dbus) || fail "the installed tray fails the D-Bus test"

# Copying onto a running program fails ("Text file busy"); install.sh must rename over it.
address=$(mktemp "$work/bus.XXXX") || cannot "no temporary file"
dbus-daemon --session --nofork --print-address=1 >"$address" &
bus=$!
for _ in $(seq 50); do [ -s "$address" ] && break; sleep 0.1; done
[ -s "$address" ] || cannot "dbus-daemon did not start"
DBUS_SESSION_BUS_ADDRESS=$(head -n 1 "$address") DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent/deskpuck-test \
    "$XDG_BIN_HOME/deskpuck" >/dev/null 2>&1 &
tray=$!
sleep 2
kill -0 "$tray" 2>/dev/null || fail "the installed tray did not keep running"
"$folder/install.sh" >/dev/null || fail "install.sh over the running tray failed"
cmp -s "$folder/deskpuck" "$XDG_BIN_HOME/deskpuck" || fail "the running tray was not replaced"
kill -0 "$tray" 2>/dev/null || fail "replacing the tray stopped the running one"

"$folder/install.sh" --uninstall >/dev/null || fail "install.sh --uninstall failed"
for file in "$XDG_BIN_HOME/deskpuck" "$XDG_BIN_HOME/deskpuck-cli" "$XDG_DATA_HOME/applications/deskpuck.desktop"; do
    [ ! -e "$file" ] || fail "$file is left after uninstalling"
done
echo "tarball.test.sh: $(basename "$tarball") installs, runs, updates in place and uninstalls"
