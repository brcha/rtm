#!/usr/bin/env bash
# Keeps the Zola that Vercel installs (the install-zola.sh arguments in vercel.json) in step with
# the Zola that flake.lock pins for the dev shell and CI: reads the pinned version, downloads that
# release's musl tarball, computes its SHA-256 and rewrites vercel.json. Run it after every
# nixpkgs bump (nix flake update); see site/AGENTS.md and doc/nix-packaging.md.
#
# Usage: scripts/site/sync-zola-version.sh

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

version=$(nix eval --raw --inputs-from . nixpkgs#zola.version)
tarball=zola-v$version-x86_64-unknown-linux-musl.tar.gz
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Zola publishes musl builds since 0.21; an older pin has none.
curl -fsSL -o "$tmp/$tarball" "https://github.com/getzola/zola/releases/download/v$version/$tarball" ||
    die "no $tarball for Zola $version"
sha256=$(sha256sum "$tmp/$tarball" | cut -d' ' -f1)

pattern='scripts/site/install-zola\.sh [^ "]+ [0-9a-f]{64}'
grep -Eq "$pattern" vercel.json || die "no install-zola.sh arguments found in vercel.json"
before=$(<vercel.json)
sed -E -i "s#$pattern#scripts/site/install-zola.sh $version $sha256#" vercel.json

if [[ $(<vercel.json) == "$before" ]]; then
    echo "vercel.json already installs Zola $version."
else
    echo "vercel.json now installs Zola $version ($sha256)."
fi
