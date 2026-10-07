#!/bin/bash
# Installs a pinned gitleaks into DIR (default ~/.local/bin), checked against a hash kept here.
# Exit 0 installed, 3 could not install (platform, download or hash).
set -uo pipefail

version=8.30.1
dir=${1:-$HOME/.local/bin}

case "$(uname -s)/$(uname -m)" in
    Darwin/arm64) platform=darwin_arm64 sha=b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5 ;;
    Linux/x86_64) platform=linux_x64 sha=551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb ;;
    *) echo "CANNOT INSTALL: no pinned gitleaks for $(uname -s)/$(uname -m)" >&2; exit 3 ;;
esac

work=$(mktemp -d) || exit 3
trap 'rm -rf "$work"' EXIT
archive="$work/gitleaks.tar.gz"
url="https://github.com/gitleaks/gitleaks/releases/download/v$version/gitleaks_${version}_$platform.tar.gz"

curl -fsSL --retry 3 -o "$archive" "$url" || { echo "CANNOT INSTALL: download failed: $url" >&2; exit 3; }
actual=$(shasum -a 256 "$archive" | cut -d' ' -f1)
if [ "$actual" != "$sha" ]; then
    echo "CANNOT INSTALL: $url has SHA-256 $actual, expected $sha" >&2
    exit 3
fi
if ! { mkdir -p "$dir" && tar -xzf "$archive" -C "$work" gitleaks && mv "$work/gitleaks" "$dir/gitleaks"; }; then
    echo "CANNOT INSTALL: could not unpack gitleaks into $dir" >&2
    exit 3
fi
echo "Installed gitleaks $version to $dir"
