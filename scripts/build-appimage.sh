#!/usr/bin/env bash
# Builds the rtmapp AppImage (x86_64) from a release binary. Used by the Release workflow; runs the
# same way locally.
#
# Usage: scripts/build-appimage.sh <rtmapp-binary> <version> <output.AppImage>
#
# The AppDir holds only the binary, the desktop entry, the icons and the licences. No libraries are
# bundled: rtmapp links nothing beyond glibc and loads Wayland, xkbcommon, EGL/Vulkan and fontconfig
# with dlopen, which the AppImage excludelist says to take from the host.
#
# appimagetool and the type2 runtime are downloaded from pinned releases and checked against their
# SHA-256 (upstream also publishes moving "continuous" builds, which this avoids). Set APPIMAGE_TOOLS_DIR
# to keep the downloads between runs. appimagetool runs with --appimage-extract-and-run, so no FUSE is
# needed.

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

[[ $# -eq 3 ]] || die "usage: ${0##*/} <rtmapp-binary> <version> <output.AppImage>"

binary=$1
version=$2
output=$3
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
assets=$root/rtmapp/assets

[[ -x $binary ]] || die "not an executable: $binary"

tool_url=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
tool_sha256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
runtime_sha256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tools=${APPIMAGE_TOOLS_DIR:-$work/tools}
mkdir -p "$tools"

# fetch <url> <sha256> <file>: downloads unless a file with the right hash is already there.
fetch() {
    if ! echo "$2  $3" | sha256sum --check --status 2>/dev/null; then
        curl -fsSL --retry 3 --max-time 300 -o "$3" "$1"
        echo "$2  $3" | sha256sum --check --quiet || die "checksum mismatch for $1"
    fi
}
fetch "$tool_url" "$tool_sha256" "$tools/appimagetool"
fetch "$runtime_url" "$runtime_sha256" "$tools/runtime-x86_64"
chmod +x "$tools/appimagetool"

appdir=$work/rtmapp.AppDir
hicolor=$appdir/usr/share/icons/hicolor
install -Dm755 "$binary" "$appdir/usr/bin/rtmapp"
install -Dm644 "$assets/rtmapp.desktop" "$appdir/usr/share/applications/rtmapp.desktop"
install -Dm644 "$assets/icons/16x16.png" "$hicolor/16x16/apps/rtmapp.png"
install -Dm644 "$assets/icons/24x24.png" "$hicolor/24x24/apps/rtmapp.png"
install -Dm644 "$assets/icons/32x32.png" "$hicolor/32x32/apps/rtmapp.png"
install -Dm644 "$assets/icons/128x128.png" "$hicolor/128x128/apps/rtmapp.png"
install -Dm644 "$assets/icons/128x128@2x.png" "$hicolor/256x256/apps/rtmapp.png"
install -Dm644 "$assets/icons/icon.png" "$hicolor/512x512/apps/rtmapp.png"
install -Dm644 "$assets/icons/rtmapp.svg" "$hicolor/scalable/apps/rtmapp.svg"
install -Dm644 "$root/LICENSE" "$appdir/usr/share/doc/rtmapp/copyright"
install -Dm644 "$assets/fonts/Inter-LICENSE.txt" "$appdir/usr/share/doc/rtmapp/Inter-LICENSE.txt"

# appimagetool takes the desktop entry and the icon (named after its Icon= key) from the AppDir root.
ln -s usr/share/applications/rtmapp.desktop "$appdir/rtmapp.desktop"
ln -s usr/share/icons/hicolor/256x256/apps/rtmapp.png "$appdir/rtmapp.png"
ln -s rtmapp.png "$appdir/.DirIcon"
ln -s usr/bin/rtmapp "$appdir/AppRun"

mkdir -p "$(dirname "$output")"
ARCH=x86_64 VERSION=$version "$tools/appimagetool" --appimage-extract-and-run \
    --runtime-file "$tools/runtime-x86_64" "$appdir" "$output"
