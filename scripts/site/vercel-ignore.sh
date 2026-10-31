#!/usr/bin/env bash
# Vercel's Ignored Build Step (vercel.json): exits 0 to skip a deployment when none of the
# website's inputs changed since the branch's last successful deployment (VERCEL_GIT_PREVIOUS_SHA),
# 1 to build. When that commit is unknown (a branch's first deployment, or older than Vercel's
# shallow clone of 10 commits) it builds: building too often is harmless, skipping wrongly is not.
# The inputs are the Site workflow's paths (.github/workflows/site.yml) without the workflow file
# itself; keep the two lists in step.
#
# Usage: scripts/site/vercel-ignore.sh

# No -e: git diff's exit status is the answer. Any failure builds (exit 1).
set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1

previous=${VERCEL_GIT_PREVIOUS_SHA-}
if [[ -z $previous ]] || ! git cat-file -e "$previous^{commit}" 2>/dev/null; then
    echo "No previous deployment in this clone: building."
    exit 1
fi

if git diff --quiet "$previous" HEAD -- site scripts/site vercel.json flake.lock \
    'screenshot_*.png' rtmapp/assets/icons/rtmapp.svg rtmapp/assets/icons/rtmapp-small.svg; then
    echo "No website changes since $previous: skipping."
    exit 0
fi
echo "Website changes since $previous: building."
exit 1
