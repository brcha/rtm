#!/usr/bin/env bash
# Writes a release's section of CHANGELOG.md with git-cliff (cliff.toml). Run it on the release
# branch (releases/vYY.M) in the release's PR, then commit CHANGELOG.md; see
# doc/release-process.md. git-cliff is in the Nix dev shell.
#
# Usage: scripts/changelog.sh <version>
#
#   version  "26.1.0" or "v26.1.0", YY.M.P, or a beta of a minor release, "26.3.0-beta.1".
#
# Ranges:
#   - Patch release (P > 0): the commits since vYY.M.(P-1), prepended to CHANGELOG.md. That tag
#     exists once the previous patch's draft release is published; until then this fails.
#   - Minor release (P = 0) and its first beta (YY.M.0-beta.1): the commits since the previous
#     release branch was cut (its merge base with HEAD), written to a new CHANGELOG.md. The
#     previous release's tag is on its own branch, not reachable from here, so it cannot be the
#     boundary. With no earlier release branch (the first CalVer release), the commits since
#     v0.1.3, the last 0.x release.
#   - A later beta (YY.M.0-beta.N, N > 1): the commits since vYY.M.0-beta.(N-1), prepended. That
#     tag exists once the previous beta's draft release is published.
#   - A minor release after betas (vYY.M.0-beta.1 exists): every commit since the cut in one
#     section, the beta tags ignored, prepended above the betas' sections.

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

[[ $# -eq 1 ]] || die "usage: ${0##*/} <version>"

version=${1#[vV]}
[[ $version =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-beta\.([0-9]+))?$ ]] ||
    die "invalid version '$1': expected YY.M.P or YY.M.0-beta.N"
major=${BASH_REMATCH[1]}
minor=${BASH_REMATCH[2]}
patch=${BASH_REMATCH[3]}
beta=${BASH_REMATCH[5]}
tag=v$version
if [[ -n $beta ]] && ((patch > 0 || 10#$beta < 1)); then
    die "invalid version '$1': betas are YY.M.0-beta.N, N from 1"
fi

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ -f CHANGELOG.md ]] && grep -qF "## [$version]" CHANGELOG.md; then
    die "CHANGELOG.md already has a section for $version"
fi

git fetch --quiet --tags origin

if ((patch > 0)); then
    previous=v$major.$minor.$((patch - 1))
    git rev-parse --quiet --verify "refs/tags/$previous" >/dev/null ||
        die "tag $previous not found; publish its draft release first"
    [[ -f CHANGELOG.md ]] || die "CHANGELOG.md not found; a patch release adds to the line's file"
    git cliff "$previous..HEAD" --tag "$tag" --prepend CHANGELOG.md
elif [[ -n $beta ]] && ((10#$beta > 1)); then
    previous=v$major.$minor.0-beta.$((10#$beta - 1))
    git rev-parse --quiet --verify "refs/tags/$previous" >/dev/null ||
        die "tag $previous not found; publish its draft release first"
    [[ -f CHANGELOG.md ]] || die "CHANGELOG.md not found; a later beta adds to the line's file"
    git cliff "$previous..HEAD" --tag "$tag" --prepend CHANGELOG.md
else
    # --prune: a deleted release branch's stale ref would otherwise move the range's start.
    git fetch --quiet --prune origin '+refs/heads/releases/*:refs/remotes/origin/releases/*'
    # The highest release branch below this one, by version (v26.10 sorts above v26.9).
    previous_branch=$(git for-each-ref --format='%(refname:lstrip=3)' 'refs/remotes/origin/releases/' |
        grep -E '^releases/v[0-9]+\.[0-9]+$' |
        { cat; echo "releases/v$major.$minor"; } |
        sort -V -u |
        grep -B1 -xF "releases/v$major.$minor" |
        grep -vxF "releases/v$major.$minor" || true)
    if [[ -n $previous_branch ]]; then
        base=$(git merge-base HEAD "origin/$previous_branch")
        echo "Since $previous_branch was cut ($base)"
    else
        base=v0.1.3
        echo "No earlier release branch: since $base"
    fi
    if [[ -z $beta ]] && git rev-parse --quiet --verify "refs/tags/v$major.$minor.0-beta.1" >/dev/null; then
        # After betas: one section for everything since the cut, above the betas' sections.
        [[ -f CHANGELOG.md ]] || die "CHANGELOG.md not found; the betas' sections belong below this one"
        git cliff "$base..HEAD" --tag "$tag" --ignore-tags 'beta' --prepend CHANGELOG.md
    else
        git cliff "$base..HEAD" --tag "$tag" -o CHANGELOG.md
    fi
fi

# The new section must have at least one entry before the next section starts.
awk -v h="## [$version]" '
    index($0, h) == 1 { in_section = 1; next }
    in_section && /^## / { exit }
    in_section && /^- / { found = 1; exit }
    END { exit !found }' CHANGELOG.md ||
    die "git-cliff wrote no entries for $version; is anything new on this branch?"
echo "CHANGELOG.md has the $version section; review and commit it."
