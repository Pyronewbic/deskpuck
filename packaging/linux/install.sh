#!/bin/sh
# Installs Deskpuck for you alone, no root needed: the programs in
# ~/.local/bin, a menu entry and the icon. Run it from the unpacked folder:
#   ./install.sh              install or update
#   ./install.sh --uninstall  remove exactly what this installed
# Exit 0 done, 1 a step failed, 2 bad usage.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
bin_dir=${XDG_BIN_HOME:-$HOME/.local/bin}
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}
desktop="$data_dir/applications/deskpuck.desktop"
icon="$data_dir/icons/hicolor/128x128/apps/deskpuck.png"

fail() { echo "install.sh: $1" >&2; exit 1; }

refresh_menu() {
    # Optional: menus also notice the change on their own.
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$data_dir/applications" 2>/dev/null || true
    fi
}

case "${1:-}" in
    "") ;;
    --uninstall)
        rm -f "$bin_dir/deskpuck" "$bin_dir/deskpuck-cli" "$desktop" "$icon"
        refresh_menu
        echo "Removed Deskpuck. Your settings stay in ${XDG_CONFIG_HOME:-$HOME/.config}/deskpuck."
        exit 0
        ;;
    *) echo "Usage: ./install.sh [--uninstall]" >&2; exit 2 ;;
esac

case "$bin_dir" in
    *%* | *\\* | *[[:cntrl:]]*) fail "cannot install in $bin_dir: a menu entry cannot name a path with %, \\ or a control character; set XDG_BIN_HOME to another folder" ;;
esac
for file in deskpuck deskpuck-cli deskpuck.desktop deskpuck.png; do
    [ -f "$here/$file" ] || fail "$file is missing; run this from the unpacked Deskpuck folder"
done

mkdir -p "$bin_dir" "$data_dir/applications" "$(dirname "$icon")" || fail "could not create the folders"
# Copied beside and renamed over, which works while Deskpuck is running
# (copying onto a running program fails with "Text file busy").
for program in deskpuck deskpuck-cli; do
    new="$bin_dir/.$program.new.$$"
    if ! cp "$here/$program" "$new" || ! chmod 755 "$new" || ! mv -f "$new" "$bin_dir/$program"; then
        rm -f "$new"
        fail "could not install $program in $bin_dir"
    fi
done
cp "$here/deskpuck.png" "$icon" || fail "could not copy the icon"

# The menu entry runs the program by its full path, since ~/.local/bin is not
# on every session's PATH. Desktop entries quote " ` and $ with a backslash,
# written \\ since string escapes are undone first; launchers do not read
# % or \ back the same, so those folders are refused (as Start at Login does).
exec_path=$(printf '%s' "$bin_dir/deskpuck" | sed 's/["`$]/\\\\&/g')
tmp="$desktop.tmp.$$"
sed "s|^Exec=.*|Exec=\"$(printf '%s' "$exec_path" | sed 's/[|&\\]/\\&/g')\"|" "$here/deskpuck.desktop" > "$tmp" ||
    { rm -f "$tmp"; fail "could not write the menu entry"; }
mv "$tmp" "$desktop" || { rm -f "$tmp"; fail "could not write the menu entry"; }
refresh_menu

echo "Installed Deskpuck: start it from your app menu, or run $bin_dir/deskpuck."
echo "The Joy-Con needs access to /dev/uinput to move the pointer; see the README if it does not."
