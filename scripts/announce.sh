#!/usr/bin/env bash
# Announces a stable release on X through Buffer's API, with the text under [extra.announce] in the
# release's website post, followed by the post's URL. See "Announcement" in doc/release-process.md;
# run by .github/workflows/announce.yml.
#
# Usage: scripts/announce.sh [--check | --dry-run] <site/content/news/X.Y.Z.md>
#
#   --check    check the text and print the post, without network access (pull requests)
#   --dry-run  also wait until the post is live, then print the requests instead of sending them
#
# A beta's post is skipped. A real run waits until the website's releases.json lists the version
# (the site is deployed by Vercel, not by the workflow), then posts. It needs BUFFER_API_KEY and
# BUFFER_CHANNEL_X.
#
# Needs curl, jq and python3 3.11 or later (tomllib reads the front matter).

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

site=https://rtm.brcha.com
# X counts every link as 23 characters, whatever its length.
x_limit=280
x_link=23

mode=send
case ${1-} in
--check) mode=check; shift ;;
--dry-run) mode=dry-run; shift ;;
esac
[[ $# -eq 1 ]] || die "usage: ${0##*/} [--check | --dry-run] <site/content/news/X.Y.Z.md>"
post=$1
[[ -f $post ]] || die "$post not found"
version=$(basename "$post" .md)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+(-beta\.[0-9]+)?$ ]] ||
    die "$post: expected a release post named X.Y.Z.md or X.Y.0-beta.N.md"

# Each network has its text under [extra.announce] and its Buffer channel in BUFFER_CHANNEL_<NAME>.
networks=(x)

# The TOML front matter, between the first two +++ lines, as JSON (dates as strings).
meta=$(awk '/^\+\+\+$/ { if (n++) exit; next } n' "$post" |
    python3 -c 'import json, sys, tomllib; print(json.dumps(tomllib.load(sys.stdin.buffer), default=str))')

if [[ $(jq -r '.extra.channel // "stable"' <<<"$meta") != stable ]]; then
    echo "$version is a beta; betas are not announced."
    exit 0
fi

declare -A text
for network in "${networks[@]}"; do
    text[$network]=$(jq -r --arg n "$network" '.extra.announce[$n] // "" | strings' <<<"$meta")
    [[ -n ${text[$network]} ]] || die "$post: no $network text under [extra.announce]"
done
# shortcut: counts code points, while X counts CJK characters and emoji double; tighten if such a
# text ever gets near the limit.
x_length=$(jq -r '.extra.announce.x | length' <<<"$meta")
((x_length + 2 + x_link <= x_limit)) ||
    die "$post: the X text is $x_length characters; with the link it must fit in $x_limit" \
        "(at most $((x_limit - 2 - x_link)))"

if [[ $mode == check ]]; then
    for network in "${networks[@]}"; do
        printf -- '--- %s ---\n%s\n\n<the post URL>\n\n' "$network" "${text[$network]}"
    done
    echo "The announcement for $version is ready."
    exit 0
fi

# Checked before waiting and before the first post, so a missing setting fails at once and never
# leaves a release half announced.
if [[ $mode == send ]]; then
    [[ -n ${BUFFER_API_KEY-} ]] || die "BUFFER_API_KEY is not set"
    for network in "${networks[@]}"; do
        channel_var=BUFFER_CHANNEL_${network^^}
        [[ -n ${!channel_var-} ]] || die "$channel_var is not set"
    done
fi

# The post is live once the deployed releases.json lists it; its entry has the post's URL.
url=
for ((attempt = 1; attempt <= 60; attempt++)); do
    url=$(curl -fsS "$site/releases.json" 2>/dev/null |
        jq -r --arg v "$version" '.releases[] | select(.version == $v) | .url' 2>/dev/null) || true
    [[ -n $url ]] && break
    echo "Waiting for $version in $site/releases.json ($attempt/60)..."
    sleep 30
done
[[ -n $url ]] || die "$version is not in $site/releases.json after 30 minutes; is the post deployed?"
curl -fsS -o /dev/null "$url" || die "$url does not answer"
echo "The post is live: $url"

# shellcheck disable=SC2016 # $input is a GraphQL variable, not a shell one.
query='mutation CreatePost($input: CreatePostInput!) {
  createPost(input: $input) {
    ... on PostActionSuccess { post { id } }
    ... on MutationError { message }
  }
}'

failed=()
for network in "${networks[@]}"; do
    channel_var=BUFFER_CHANNEL_${network^^}
    channel=${!channel_var:-<$channel_var>}
    body=$(jq -n --arg query "$query" --arg text "${text[$network]}"$'\n\n'"$url" \
        --arg channel "$channel" '{
            query: $query,
            variables: {input: {
                text: $text,
                channelId: $channel,
                schedulingType: "automatic",
                mode: "shareNow"
            }}
        }')
    if [[ $mode == dry-run ]]; then
        printf -- '--- %s ---\n%s\n' "$network" "$body"
        continue
    fi
    # GraphQL answers 200 for its own errors; they are in the body.
    if ! response=$(curl -fsS https://api.buffer.com \
        -H "Authorization: Bearer $BUFFER_API_KEY" \
        -H 'Content-Type: application/json' \
        --data-binary "$body"); then
        echo "error: $network: the request to Buffer failed" >&2
        failed+=("$network")
        continue
    fi
    id=$(jq -r '.data.createPost.post.id // empty' <<<"$response" 2>/dev/null) || id=
    if [[ -z $id ]]; then
        echo "error: $network: Buffer did not post it: $(jq -c '.errors // .data' <<<"$response" 2>/dev/null ||
            echo "$response")" >&2
        failed+=("$network")
        continue
    fi
    echo "$network: posted ($id)"
done

if ((${#failed[@]} > 0)); then
    die "not announced on: ${failed[*]}"
fi
if [[ $mode == dry-run ]]; then
    echo "Dry run: nothing was posted."
else
    echo "$version is announced."
fi
