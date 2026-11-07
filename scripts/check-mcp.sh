#!/usr/bin/env bash
# Checks that an rtm MCP server answers the MCP handshake over stdin and stdout: the server is
# named rtm, it offers list_items, and its list_files names the expected todo.txt, which proves the
# file came from where the platform keeps it (-f, or rtmapp's config). Used by the Release workflow
# on every package; runs the same way anywhere. Bash 3.2 (macOS), no python, jq or timeout.
#
# Usage: scripts/check-mcp.sh <expected path> <command> [<argument>...]
#   scripts/check-mcp.sh "$PWD/todo.txt" rtmcli -f "$PWD/todo.txt" mcp
#   scripts/check-mcp.sh /home/me/todo.txt rtmapp mcp
#
# Run one server at a time: servers with the same parent process count as one client, and a second
# one running at once offers no tools.

set -euo pipefail

if [ $# -lt 2 ]; then
    echo "usage: ${0##*/} <expected path> <command> [<argument>...]" >&2
    exit 2
fi
expected=$1
shift

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
out=$scratch/stdout
err=$scratch/stderr
: > "$out"

# The requests at once; stdin stays open until the last reply arrives (or 20 s pass), as a client
# keeps it open, then closes so the server exits. Reading the server's output while it is written
# is the point.
# shellcheck disable=SC2094
{
    printf '%s\n' \
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check-mcp","version":"0"}}}' \
        '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
        '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
        '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_files","arguments":{}}}'
    for _ in $(seq 100); do
        # Gone once the check is over, if the server exited early.
        [ -f "$out" ] || break
        grep -q '"id":3' "$out" && break
        sleep 0.2
    done
} | "$@" > "$out" 2> "$err" &
server=$!
# A server that never exits is killed, and the check fails.
(sleep 30 && kill "$server") > /dev/null 2>&1 &
watchdog=$!

status=0
wait "$server" || status=$?
kill "$watchdog" 2> /dev/null || true

fail() {
    echo "::error::$*"
    echo "--- stdout:"
    cat "$out"
    echo "--- stderr:"
    cat "$err"
    exit 1
}
[ "$status" = 0 ] || fail "$1 exited with status $status"
grep -q '"name":"rtm"' "$out" || fail "$1 did not answer initialize as rtm"
grep -q '"list_items"' "$out" || fail "$1 offers no list_items tool"
grep -F -q "$expected" "$out" || fail "$1 does not serve $expected"
echo "MCP check passed: $* serves $expected"
