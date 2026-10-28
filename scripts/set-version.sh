#!/usr/bin/env bash
# Manual pre-release tool: sets the release version across all version-carrying manifests in
# the workspace. Run this locally, commit the result, and only then tag the release.
# This is the bash counterpart of set-version.ps1; keep the two in step.
#
# Usage: scripts/set-version.sh <version> [repo-root]
#
#   version    "26.1.0" or "v26.1.0" (a leading v/V is stripped). Exactly major.minor.patch,
#              all-numeric: MSI ProductVersion cannot represent pre-release or build suffixes.
#   repo-root  Root of the workspace. Defaults to the parent of this script's directory.
#
# The release workflow (.github/workflows/release.yml) treats the committed manifests as the
# single source of truth for the release version: it never writes to them, and on a tag push it
# fails if the tag does not match what is committed. This script makes that true.
#
# Rewrites the `version` field in:
#   - rtmapp/src-tauri/tauri.conf.json  (drives MSI ProductVersion and exe metadata)
#   - rtmapp/src-tauri/Cargo.toml
#   - rtmcli/Cargo.toml
#   - rtmgui/Cargo.toml
#   - todotxt/Cargo.toml
#   - rtmapp/package.json               (cosmetic, kept in sync for consistency)
#
# In a Cargo.toml only the `version` key inside the `[package]` table is touched:
# todotxt/Cargo.toml also has a `[dependencies.uuid]` table with its own `version` line.
# In the JSON files only the top-level (2-space-indented) key is touched. Files are patched
# line by line rather than parsed, so hand-maintained formatting is left alone.
#
# A target whose version key cannot be found is an error, never a silent skip: a stale version
# in a release would break MSI upgrade detection. No git operations; running it twice with the
# same version is a no-op. Cargo.lock files are not refreshed; a cargo build does that.

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

[[ $# -ge 1 && $# -le 2 ]] || die "usage: ${0##*/} <version> [repo-root]"

version=$1
root=${2:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}

normalised=${version#[vV]}
[[ $normalised =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] ||
    die "invalid version '$version': expected 'major.minor.patch' (optionally prefixed with 'v'), all-numeric, no pre-release or build-metadata suffix"
major=${BASH_REMATCH[1]}
minor=${BASH_REMATCH[2]}
patch=${BASH_REMATCH[3]}

# MSI ProductVersion limits: https://learn.microsoft.com/windows/win32/msi/productversion
# The length check keeps the arithmetic below from overflowing; 10# avoids octal on "08".
check_max() {
    local name=$1 value=$2 max=$3
    if ((${#value} > 9 || 10#$value > max)); then
        die "version '$normalised' invalid: $name ($value) exceeds the MSI ProductVersion limit of $max"
    fi
}
check_max major "$major" 255
check_max minor "$minor" 255
check_max patch "$patch" 65535

# kind is "toml" (the `version` of [package]) or "json" (the top-level "version").
# mode "get" prints the current line; mode "apply" prints the patched file. Only the matched
# text is replaced, so line endings stay as they were. Exits 1 when the key is not found.
patch_awk='
BEGIN {
    if (kind == "toml") {
        re = "^version = \"[^\"]*\""
        new = "version = \"" v "\""
    } else {
        re = "^  \"version\": \"[^\"]*\""
        new = "  \"version\": \"" v "\""
    }
    in_scope = (kind != "toml")
}
kind == "toml" && /^\[/ { in_scope = ($0 ~ /^\[package\]/) }
in_scope && !found && match($0, re) {
    found = 1
    old = substr($0, RSTART, RLENGTH)
    if (mode == "apply") sub(re, new)
}
mode == "apply" { print }
END {
    if (!found) exit 1
    if (mode == "get") print old
}'

run_awk() {
    awk -v kind="$1" -v v="$normalised" -v mode="$2" "$patch_awk" "$3"
}

targets=(
    "json rtmapp/src-tauri/tauri.conf.json"
    "toml rtmapp/src-tauri/Cargo.toml"
    "toml rtmcli/Cargo.toml"
    "toml rtmgui/Cargo.toml"
    "toml todotxt/Cargo.toml"
    "json rtmapp/package.json"
)

echo "Setting version to $normalised"

# Check every target before writing any, so a failure leaves the tree untouched.
for target in "${targets[@]}"; do
    read -r kind rel <<<"$target"
    path=$root/$rel
    [[ -f $path ]] || die "version target not found: '$path'"
    run_awk "$kind" get "$path" >/dev/null ||
        die "version key not found in '$path'; the file may have been reformatted or the key moved"
done

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

for target in "${targets[@]}"; do
    read -r kind rel <<<"$target"
    path=$root/$rel
    old=$(run_awk "$kind" get "$path")
    run_awk "$kind" apply "$path" >"$tmp"
    cat "$tmp" >"$path" # not mv, so the file's mode and ownership are kept
    if [[ $old == *"\"$normalised\""* ]]; then
        echo "  $path : already at $normalised (no change)"
    else
        echo "  $path : $old -> ${old%\"*\"}\"$normalised\""
    fi
done

echo "Version set to $normalised across ${#targets[@]} file(s)."
