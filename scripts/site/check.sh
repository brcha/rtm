#!/usr/bin/env bash
# Builds the website and checks its links: Zola's own check (templates, internal links in the
# Markdown), then lychee over the built pages, which also sees the images, stylesheets and anchors
# the templates refer to. It also checks releases.json, the release metadata the programs read
# (RTM-135). By default no network is used; with --external, external links are checked instead.
# Needs zola, lychee and jq (the dev shell has them); CI runs this script, see
# .github/workflows/site.yml and site/AGENTS.md.
#
# Usage: scripts/site/check.sh [--external]

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

external=false
case $# in
    0) ;;
    1) [[ $1 == --external ]] || die "usage: ${0##*/} [--external]"; external=true ;;
    *) die "usage: ${0##*/} [--external]" ;;
esac

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

scripts/site/prepare.sh
zola --root site check --skip-external-links
zola --root site build

# releases.json: valid JSON in the documented shape, and the landing page's release among its
# stable entries (not necessarily the first: a patch on an older line can be newer by date).
releases=site/public/releases.json
[[ -f $releases ]] || die "$releases was not generated"
jq -e '.schema == 1 and (.releases | type == "array" and length > 0) and all(.releases[];
        all(.version, .channel, .date, .url; type == "string" and length > 0))' \
    "$releases" > /dev/null || die "$releases is not in the documented shape"
latest=$(sed -n 's|.*<p>Version \([^ ]*\)\. The desktop app.*|\1|p' site/public/index.html)
[[ -n $latest ]] || die "no version in the landing page's download section"
jq -e --arg v "$latest" 'any(.releases[]; .version == $v and .channel == "stable")' \
    "$releases" > /dev/null || die "$releases has no stable entry for $latest, the landing page's release"

# The pages use absolute URLs under base_url; map them to the built files, or lychee would treat
# them as remote and skip them offline.
base_url=$(sed -n 's/^base_url = "\(.*\)"$/\1/p' site/config.toml)
[[ -n $base_url ]] || die "no base_url in site/config.toml"
remap="^${base_url//./\\.}/(.*) file://$PWD/site/public/\$1"
# A directory link (the site root, a page's pretty URL) means its index.html, as on the server;
# that also lets lychee check fragments such as /#download.
local_links=(--remap "$remap" --index-files index.html)
# lychee skips .json files when it scans a directory; the release URLs are checked by naming the file.
inputs=(site/public "$releases")

if $external; then
    # No fragment checks: GitHub renders README headings with user-content- ids, so anchors on
    # github.com would fail falsely.
    lychee --no-progress "${local_links[@]}" "${inputs[@]}"
else
    lychee --no-progress --offline --include-fragments "${local_links[@]}" "${inputs[@]}"
fi
