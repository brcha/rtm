#!/usr/bin/env bash
# The LLM eval of rtm's MCP tools (rtmmcp/eval, see rtmmcp/AGENTS.md): runs each scenario a few
# times with Claude Code and the rtm server, on scratch files only, and prints the passes per
# scenario. Needs node 22.22 or newer (`nix develop`) and a Claude Code login.
#
# Usage: scripts/mcp-eval.sh
#
#   RTM_EVAL_RTMAPP        absolute path of the rtmapp to test (default: `nix build .#rtmapp`;
#                          never one from PATH)
#   RTM_EVAL_CLAUDE_MODEL  Claude model (default claude-sonnet-5-5)
#   RTM_EVAL_REPEAT        runs per scenario (default 3)
#   RTM_EVAL_MIN_PASSES    passes each scenario needs (default 2)
#
# Exits 1 when a scenario has fewer passes than that. The full results, with the model's answers
# and tool calls, are in target/mcp-eval/results.json; `npx promptfoo@0.125.1 view` shows them.
# Bash 3.2 (macOS).

set -euo pipefail

promptfoo=promptfoo@0.125.1
sdk=@anthropic-ai/claude-agent-sdk@0.3.319

root=$(cd "$(dirname "$0")/.." && pwd)
repeat=${RTM_EVAL_REPEAT:-3}
min=${RTM_EVAL_MIN_PASSES:-2}
export RTM_EVAL_CLAUDE_MODEL=${RTM_EVAL_CLAUDE_MODEL:-claude-sonnet-5-5}

fail() { echo "mcp-eval: $*" >&2; exit 2; }

command -v npx >/dev/null || fail "needs node 22.22 or newer (nix develop)"
command -v jq >/dev/null || fail "needs jq (nix develop)"

if [ -n "${RTM_EVAL_RTMAPP:-}" ]; then
  case $RTM_EVAL_RTMAPP in /*) ;; *) fail "RTM_EVAL_RTMAPP must be an absolute path" ;; esac
  [ -x "$RTM_EVAL_RTMAPP" ] || fail "$RTM_EVAL_RTMAPP is not an executable"
else
  echo "Building rtmapp from the checkout..." >&2
  nix build "$root#rtmapp" --out-link "$root/target/mcp-eval/rtmapp"
  RTM_EVAL_RTMAPP=$root/target/mcp-eval/rtmapp/bin/rtmapp
fi
export RTM_EVAL_RTMAPP

RTM_EVAL_DIR=$(mktemp -d "${TMPDIR:-/tmp}/rtm-mcp-eval.XXXXXX")
export RTM_EVAL_DIR
trap 'rm -rf "$RTM_EVAL_DIR"' EXIT

out=$root/target/mcp-eval/results.json
mkdir -p "$(dirname "$out")"
rm -f "$out"

# The Claude Code login (subscription) rather than an API key from the environment, which
# Claude Code would prefer and bill. No telemetry or update checks from promptfoo.
(
  cd "$root/rtmmcp/eval"
  env -u ANTHROPIC_API_KEY PROMPTFOO_DISABLE_TELEMETRY=1 PROMPTFOO_DISABLE_UPDATE=1 \
    npx --yes -p "$promptfoo" -p "$sdk" promptfoo eval \
    --config promptfooconfig.yaml --no-cache --max-concurrency 1 --repeat "$repeat" \
    --output "$out" --no-progress-bar
) || true # promptfoo exits non-zero when a test fails; the table below decides.

[ -s "$out" ] || fail "promptfoo wrote no results"

jq -r --argjson min "$min" '
  .results.results
  | group_by([.provider.label, .testCase.description])
  | map({
      client: .[0].provider.label,
      scenario: .[0].testCase.description,
      passes: (map(select(.success)) | length),
      runs: length,
      cost: (map(.cost // 0) | add),
      reasons: (map(select(.success | not) | .gradingResult.reason // .error // "") | unique)
    })
  | (map("\(if .passes >= $min then "pass" else "FAIL" end)  \(.client)  \(.passes)/\(.runs)  $\(.cost * 100 | round / 100)  \(.scenario)"
         + (if .reasons == [] then "" else "\n      " + (.reasons | join("\n      ")) end))
     | join("\n")),
    "total \(map(.cost) | add * 100 | round / 100) USD (API-equivalent estimate)",
    (if all(.passes >= $min) then "" else "FAILED: a scenario passed fewer than \($min) times" end)
' "$out"

jq -e --argjson min "$min" '
  .results.results | group_by([.provider.label, .testCase.description])
  | all((map(select(.success)) | length) >= $min)
' "$out" >/dev/null
