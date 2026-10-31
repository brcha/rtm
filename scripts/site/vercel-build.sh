#!/usr/bin/env bash
# Builds the website on Vercel (the build step in vercel.json), with the Zola that
# scripts/site/install-zola.sh put in the repository root. The site's URLs are absolute, so a
# preview deployment is built with its own *.vercel.app address as the base URL; otherwise it would
# load the production site's stylesheet and images instead of its own. Production, and any build
# without Vercel's variables, keeps base_url from site/config.toml. See site/AGENTS.md.
#
# Usage: scripts/site/vercel-build.sh

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

scripts/site/prepare.sh

base_url=()
if [[ -n ${VERCEL_ENV-} && $VERCEL_ENV != production && -n ${VERCEL_URL-} ]]; then
    base_url=(--base-url "https://$VERCEL_URL")
fi

./zola --root site build "${base_url[@]}"
