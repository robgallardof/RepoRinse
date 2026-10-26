#!/bin/sh
# Offline installation from an extracted archive. No shell startup files are changed.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
binary=${1:-"$script_dir/../reporinse"}
destination=${2:-"$HOME/.local/bin"}
test -f "$binary"
mkdir -p "$destination"
cp "$binary" "$destination/reporinse"
chmod 755 "$destination/reporinse"
printf 'Installed to %s/reporinse. Add this directory to PATH if needed.\n' "$destination"
