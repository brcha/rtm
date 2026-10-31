#!/usr/bin/env bash
# Downloads Zola's static (musl) Linux x86-64 binary into the repository root and checks it
# against a SHA-256. Vercel runs it as the install step (vercel.json): its built-in Zola preset
# fails on current Zola, whose glibc build needs a newer glibc than Vercel's build image has.
# The version and checksum in vercel.json follow flake.lock; update them with
# scripts/site/sync-zola-version.sh. See site/AGENTS.md.
#
# Usage: scripts/site/install-zola.sh <version> <sha256>

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

[[ $# -eq 2 ]] || die "usage: ${0##*/} <version> <sha256>"
version=$1
sha256=$2

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

tarball=zola-v$version-x86_64-unknown-linux-musl.tar.gz
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl -fsSL -o "$tmp/$tarball" "https://github.com/getzola/zola/releases/download/v$version/$tarball"
echo "$sha256  $tmp/$tarball" | sha256sum -c --quiet - || die "checksum mismatch for $tarball"
tar -xzf "$tmp/$tarball" -C . zola
./zola --version
