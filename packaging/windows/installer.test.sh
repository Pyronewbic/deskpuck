#!/bin/bash
# Usage: packaging/windows/installer.test.sh SETUP_EXE (Git Bash on Windows, for CI runners:
# it installs and uninstalls Deskpuck for this user). Installs with the flags the one-click
# update uses, checks the installed copy starts, installs again over the running copy, then
# uninstalls. Exit 0 every case behaves, 1 a case failed, 2 bad usage, 3 could not run.
set -uo pipefail
# Git Bash would rewrite the installer's /FLAGS and reg.exe's /v into paths.
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'

fail() { echo "FAIL: $1" >&2; exit 1; }
cannot() { echo "installer.test.sh: CANNOT CHECK: $1" >&2; exit 3; }

[ $# -eq 1 ] && [ -f "$1" ] || { echo "Usage: packaging/windows/installer.test.sh SETUP_EXE" >&2; exit 2; }
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) ;;
    *) cannot "runs on Windows" ;;
esac
setup=$1
[[ "$(basename "$1")" =~ ^Deskpuck-([0-9]+\.[0-9]+\.[0-9]+)-setup\.exe$ ]] || cannot "not a Deskpuck installer name"
version=${BASH_REMATCH[1]}
uninstall_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{F0FA84CD-5402-434B-9A3E-9C18C110CD17}_is1'
run_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Run'
work=$(mktemp -d) || cannot "no temporary folder"
trap 'rm -rf "$work"' EXIT

value() { # key, name: the REG_SZ value, or nothing
    reg query "$1" /v "$2" 2>/dev/null | tr -d '\r' | sed -n "s/^ *$2 *REG_SZ *//p"
}
# PIDs of deskpuck.exe running from this path (PowerShell compares paths without case).
running() {
    powershell -NoProfile -Command \
        "Get-Process deskpuck -ErrorAction Ignore | Where-Object Path -eq '$1' | ForEach-Object Id" | tr -d '\r'
}
eventually() { # seconds, description, command...
    local seconds=$1 what=$2
    shift 2
    for _ in $(seq "$seconds"); do
        "$@" && return 0
        sleep 1
    done
    fail "timed out after ${seconds}s waiting for $what"
}
update() { # the one-click update's flags (deskpuck-tray/src/update.rs), logged
    "$setup" /SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS "/LOG=$(cygpath -w "$work/$1.log")"
}

[ -z "$(value "$uninstall_key" DisplayVersion)" ] || cannot "Deskpuck is already installed for this user"

update install || fail "the installer failed (log: $(tail -5 "$work/install.log"))"
[ "$(value "$uninstall_key" DisplayVersion)" = "$version" ] || fail "the uninstall entry does not say $version"
folder=$(value "$uninstall_key" InstallLocation)
folder=${folder%\\}
exe="$folder\\deskpuck.exe"
if [ -z "$folder" ] || [ ! -f "$(cygpath -u "$exe")" ]; then
    fail "no deskpuck.exe where the installer says it put it: $exe"
fi

# deskpuck.iss starts the new copy even when silent; that is how an update comes back.
started() { first=$(running "$exe"); [ -n "$first" ]; }
eventually 60 "the installer to start Deskpuck" started

update reinstall || fail "installing over the running copy failed (log: $(tail -5 "$work/reinstall.log"))"
replaced() {
    local now
    now=$(running "$exe")
    [ -n "$now" ] && ! grep -qx "$first" <<<"$now"
}
eventually 60 "the running copy to be replaced by a new one" replaced

reg add "$run_key" /v Deskpuck /t REG_SZ /d "\"$exe\"" /f >/dev/null || cannot "could not set Start at Login"
for pid in $(running "$exe"); do taskkill /F /PID "$pid" >/dev/null 2>&1; done
"$(cygpath -u "$folder\\unins000.exe")" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART || fail "the uninstaller failed"
# The uninstaller hands over to a copy of itself and returns at once.
removed() {
    [ -z "$(value "$uninstall_key" DisplayVersion)" ] && [ ! -e "$(cygpath -u "$exe")" ] &&
        [ -z "$(value "$run_key" Deskpuck)" ]
}
eventually 60 "the uninstaller to remove the files, its entry and Start at Login" removed

echo "installer.test.sh: $(basename "$1") installs, starts, updates a running copy and uninstalls"
