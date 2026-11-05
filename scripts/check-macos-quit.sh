#!/usr/bin/env bash
# Checks that quitting the installed macOS app the way Cmd+Q does runs its exit sync: the app is
# asked to quit through NSApplication terminate: (the quit Apple event by default, a real Cmd+Q with
# --keystroke), and a commit only the exit sync pushes must reach the remote. Used by the Release
# workflow; runs the same way on any Mac.
#
# Usage: scripts/check-macos-quit.sh [--keystroke] [<app>]
#
# <app> defaults to "/Applications/Rusty Todo.txt Manager.app". Everything lives in a scratch
# directory: the app runs with HOME set there, so its config (dirs::config_dir()) and git's global
# config are scratch too, and the user's real ones are never read or written. Sending the quit event
# may need Automation permission for the terminal; --keystroke needs Accessibility permission.
#
# Exit status: 0 passed, 1 failed, 2 inconclusive (the startup sync pushed the marker before the
# quit, so the result proves nothing).

set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

usage="usage: ${0##*/} [--keystroke] [<app>]"
keystroke=false
app="/Applications/Rusty Todo.txt Manager.app"
for arg in "$@"; do
    case $arg in
        --keystroke) keystroke=true ;;
        -*) die "$usage" ;;
        *) app=$arg ;;
    esac
done

[[ $(uname -s) == Darwin ]] || die "runs on macOS only"
[[ -d $app ]] || die "no app at $app"
bundle_id=$(plutil -extract CFBundleIdentifier raw -o - "$app/Contents/Info.plist")

# The quit event goes to the app by bundle id, so a running instance (the user's own, or one a
# previous step is still stopping) could be the one that quits.
for _ in $(seq 10); do
    pgrep -x rtmapp > /dev/null || break
    sleep 0.5
done
pgrep -x rtmapp > /dev/null && die "rtmapp is already running; quit it first"

# Resolved: /var is a link to /private/var, and git reports the resolved path.
scratch=$(cd "$(mktemp -d)" && pwd -P)
home=$scratch/home
log=$scratch/stderr.log
pid=
cleanup() {
    [[ -n $pid ]] && kill "$pid" 2> /dev/null
    rm -rf "$scratch"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    if [[ -s $log ]]; then
        echo '--- rtmapp stderr ---' >&2
        cat "$log" >&2
    fi
    exit 1
}

# The script's own git uses the scratch HOME too, so the user's global config (signing, hooks,
# templates) never applies to these commits.
g() {
    HOME=$home GIT_CONFIG_NOSYSTEM=1 git "$@"
}

# Captured first: grep -q in a pipe can SIGPIPE git log, which pipefail reports as a miss.
remote_has_marker() {
    local subjects
    subjects=$(g -C "$scratch/remote" log --format=%s main)
    grep -qx "quit marker" <<< "$subjects"
}

isolate() {
    g -C "$1" config user.name "rtm quit check"
    g -C "$1" config user.email rtm@example.com
    g -C "$1" config commit.gpgsign false
    g -C "$1" config core.hooksPath "$scratch/no hooks"
    g -C "$1" config rerere.enabled false
}

mkdir -p "$home/Library/Application Support/rtm" "$scratch/no hooks"
g init --quiet --bare --initial-branch=main "$scratch/remote"
g init --quiet --initial-branch=main "$scratch/app"
isolate "$scratch/app"
printf '(A) Quit check +rtm\n' > "$scratch/app/todo.txt"
g -C "$scratch/app" add todo.txt
g -C "$scratch/app" commit --quiet -m start
g -C "$scratch/app" remote add origin "$scratch/remote"
g -C "$scratch/app" push --quiet -u origin main
g clone --quiet -c core.hooksPath="$scratch/no hooks" "$scratch/remote" "$scratch/other"
isolate "$scratch/other"

# A commit the app does not have yet: its startup pull brings it in, which shows the app is running
# on the scratch config (so HOME reached it) and that the startup sync has run.
echo startup > "$scratch/other/startup.txt"
g -C "$scratch/other" add startup.txt
g -C "$scratch/other" commit --quiet -m "startup marker"
g -C "$scratch/other" push --quiet
startup=$(g -C "$scratch/other" rev-parse HEAD)

printf 'file_name = "%s"\nsync_interval_minutes = 0\n' "$scratch/app/todo.txt" \
    > "$home/Library/Application Support/rtm/config.toml"

open -n -a "$app" --env HOME="$home" --env GIT_CONFIG_NOSYSTEM=1 \
    --env RUST_LOG=info,zbus=warn,tracing=warn --stderr "$log"
for _ in $(seq 20); do
    pid=$(pgrep -nx rtmapp) && break
    sleep 0.5
done
[[ -n $pid ]] || fail "rtmapp did not start"
echo "rtmapp started (pid $pid)"

synced=false
for _ in $(seq 60); do
    if g -C "$scratch/app" merge-base --is-ancestor "$startup" HEAD 2> /dev/null; then
        synced=true
        break
    fi
    kill -0 "$pid" 2> /dev/null || fail "rtmapp exited before its startup sync"
    sleep 0.5
done
$synced || fail "rtmapp did not pull at startup within 30 s: either it did not get HOME from open --env and read another config, or its startup sync failed"
echo "Startup sync pulled the startup marker."
# The pull is followed by a push of nothing; let it finish before committing the marker.
sleep 2

echo quit > "$scratch/app/quit.txt"
g -C "$scratch/app" add quit.txt
g -C "$scratch/app" commit --quiet -m "quit marker"
sleep 1
if remote_has_marker; then
    echo "INCONCLUSIVE: the quit marker reached the remote before the quit" >&2
    exit 2
fi

# Addressed by bundle id, so each form first checks that the app runs: telling a stopped app to
# activate would start the installed copy on the user's real config. The keystroke goes to the
# frontmost app, so it is sent only when that is rtmapp.
kill -0 "$pid" 2> /dev/null || fail "rtmapp exited before the quit"
if $keystroke; then
    quit=(-e "if application id \"$bundle_id\" is running then"
        -e "tell application id \"$bundle_id\" to activate"
        -e 'delay 1'
        -e 'tell application "System Events"'
        -e 'set frontId to bundle identifier of first process whose frontmost is true'
        -e "if frontId is not \"$bundle_id\" then error \"rtmapp is not frontmost (\" & frontId & \")\""
        -e 'keystroke "q" using command down'
        -e 'end tell'
        -e 'end if')
else
    quit=(-e "if application id \"$bundle_id\" is running then tell application id \"$bundle_id\" to quit")
fi
# In the background with a limit: a permission prompt nobody answers must not hang the check.
osascript "${quit[@]}" &
osa=$!
for _ in $(seq 40); do
    kill -0 "$osa" 2> /dev/null || break
    sleep 0.5
done
if kill -0 "$osa" 2> /dev/null; then
    kill "$osa"
    fail "osascript did not return within 20 s (waiting for a permission prompt?)"
fi
wait "$osa" || fail "osascript could not send the quit (error -1743 means no Automation permission; --keystroke needs Accessibility permission)"
echo "Quit sent."

for _ in $(seq 30); do
    kill -0 "$pid" 2> /dev/null || break
    sleep 0.5
done
kill -0 "$pid" 2> /dev/null && fail "rtmapp is still running 15 s after the quit"
pid=

if remote_has_marker; then
    echo "PASS: quitting rtmapp ran the exit sync (the quit marker reached the remote)."
else
    fail "rtmapp quit, but the quit marker did not reach the remote: the exit sync did not run or did not push"
fi
