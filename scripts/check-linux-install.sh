#!/usr/bin/env bash
# Checks what the rtmapp and rtmcli packages installed, the same for every Linux package (debs and
# RPMs): the binaries, the desktop entry, the icons and a portal backend with a file chooser, then
# both MCP servers with check-mcp.sh (next to this script). Used by the Release workflow's package
# jobs, after both packages are installed. Needs bash, coreutils and grep only.
#
# Usage: scripts/check-linux-install.sh <scratch dir>
#   The MCP check writes <scratch dir>/todo.txt and <scratch dir>/config/rtm/config.toml.

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "usage: ${0##*/} <scratch dir>" >&2
    exit 2
fi
scratch=$1

fail() {
    echo "::error::$*"
    exit 1
}

rtmcli --help > /dev/null
[ -x /usr/bin/rtmapp ] || fail "/usr/bin/rtmapp missing or not executable after install."
grep -q '^Exec=rtmapp' /usr/share/applications/rtmapp.desktop \
    || fail "/usr/share/applications/rtmapp.desktop missing or without Exec=rtmapp."
for size in 16x16 24x24 32x32 128x128 256x256 512x512; do
    [ -f "/usr/share/icons/hicolor/$size/apps/rtmapp.png" ] \
        || fail "Icon missing after install: hicolor/$size/apps/rtmapp.png"
done
grep -l org.freedesktop.impl.portal.FileChooser /usr/share/xdg-desktop-portal/portals/*.portal \
    || fail "No installed XDG portal backend implements the file chooser."

# rtmcli on -f, rtmapp on the files of its config, from a scratch XDG_CONFIG_HOME. One after the
# other: servers with the same parent count as one client.
mkdir -p "$scratch/config/rtm"
printf 'Buy milk\n' > "$scratch/todo.txt"
printf 'file_name = "%s"\n' "$scratch/todo.txt" > "$scratch/config/rtm/config.toml"
here=$(dirname "$0")
bash "$here/check-mcp.sh" "$scratch/todo.txt" /usr/bin/rtmcli -f "$scratch/todo.txt" mcp
XDG_CONFIG_HOME="$scratch/config" bash "$here/check-mcp.sh" "$scratch/todo.txt" /usr/bin/rtmapp mcp
