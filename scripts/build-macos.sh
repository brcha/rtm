#!/usr/bin/env bash
# Builds the rtmapp macOS bundle and its dmg from a release binary. Used by the Release workflow; runs
# the same way on any Mac.
#
# Usage: scripts/build-macos.sh <rtmapp-binary> <version> <output.dmg>
#
# The binary is used as given (the Release workflow passes a universal one, joined with lipo). The
# bundle holds the binary, Info.plist, the icon and the licences. It gets an ad-hoc signature (arm64
# code must be signed to run; there is no Developer ID, so Gatekeeper still asks on first launch). The
# dmg holds the bundle and a link to /Applications. Only tools that come with macOS are used.

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

[[ $# -eq 3 ]] || die "usage: ${0##*/} <rtmapp-binary> <version> <output.dmg>"
[[ $(uname -s) == Darwin ]] || die "runs on macOS only"

binary=$1
version=$2
output=$3
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
assets=$root/rtmapp/assets
name="Rusty Todo.txt Manager"

[[ -x $binary ]] || die "not an executable: $binary"
# Info.plist versions are numeric; also keeps the sed below safe.
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "version must look like 1.2.3, got '$version'"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app=$work/dmg/$name.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/rtmapp"
chmod 755 "$app/Contents/MacOS/rtmapp"
sed "s/@VERSION@/$version/g" "$assets/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
cp "$assets/icons/icon.icns" "$app/Contents/Resources/rtmapp.icns"
cp "$root/LICENSE" "$assets/fonts/Inter-LICENSE.txt" "$app/Contents/Resources/"

# codesign refuses resource forks and Finder info ("detritus not allowed").
xattr -cr "$app"
codesign --force --sign - --timestamp=none "$app"

ln -s /Applications "$work/dmg/Applications"
mkdir -p "$(dirname "$output")"
# hdiutil occasionally fails with "Resource busy" on CI runners; a retry succeeds.
for attempt in 1 2 3; do
    if hdiutil create -volname "$name" -srcfolder "$work/dmg" -fs HFS+ -format UDZO -ov "$output"; then
        exit 0
    fi
    echo "hdiutil create failed (attempt $attempt)" >&2
    sleep 5
done
die "hdiutil create failed three times"
