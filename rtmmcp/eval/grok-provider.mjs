// A promptfoo provider that runs a prompt with Grok Build in headless mode (`grok -p`) and the rtm
// server, on the user's Grok login. It returns the tool calls in the shape of promptfoo's Claude
// Agent SDK provider, so checks.mjs applies to both.
//
// Grok sees only rtm's tools:
// - no built-in tools: `--tools search_tool,use_tool` leaves only the gateway through which Grok
//   reaches MCP tools (`use_tool` with `rtm__<tool>`);
// - nothing Grok would import from Claude Code, Cursor or Codex (plugins, hooks, skills, rules
//   such as CLAUDE.md, MCP servers): it runs with a scratch HOME and XDG_CONFIG_HOME, and with
//   GROK_HOME set to the user's own Grok home, so it keeps the login and the trusted folders;
// - the rtm server and the permission rules come from a project config in a directory of its own
//   outside the repository (so no AGENTS.md reaches the model), which `--trust` lets Grok load;
//   the rules allow rtm's tools but delete_item, and `dontAsk` refuses everything else.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const grokHome = process.env.GROK_HOME || path.join(os.homedir(), '.grok');

// A fixed directory, so Grok's list of trusted folders gains one entry, not one per run.
const project = path.join(
  process.env.XDG_CACHE_HOME || path.join(os.homedir(), '.cache'),
  'rtm-mcp-eval',
  'grok',
);

function writeProject() {
  const dir = process.env.RTM_EVAL_DIR;
  const env = {
    PATH: process.env.PATH,
    HOME: path.join(dir, 'home'),
    XDG_CONFIG_HOME: path.join(dir, 'home', '.config'),
    GIT_CONFIG_NOSYSTEM: '1',
  };
  const config = [
    '[mcp_servers.rtm]',
    `command = ${JSON.stringify(process.env.RTM_EVAL_RTMAPP)}`,
    'args = ["mcp"]',
    'enabled = true',
    '',
    '[mcp_servers.rtm.env]',
    ...Object.entries(env).map(([key, value]) => `${key} = ${JSON.stringify(value)}`),
    '',
    '[permission]',
    'allow = ["MCPTool(rtm__*)"]',
    'deny = ["MCPTool(rtm__delete_item)"]',
    '',
  ].join('\n');
  fs.mkdirSync(path.join(project, '.grok'), { recursive: true });
  fs.writeFileSync(path.join(project, '.grok', 'config.toml'), config);
}

function run(args) {
  const home = path.join(process.env.RTM_EVAL_DIR, 'grok-home');
  fs.mkdirSync(home, { recursive: true });
  return new Promise((resolve, reject) => {
    const child = spawn('grok', args, {
      cwd: project,
      env: {
        ...process.env,
        HOME: home,
        XDG_CONFIG_HOME: path.join(home, '.config'),
        GROK_HOME: grokHome,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (data) => (stdout += data));
    child.stderr.on('data', (data) => (stderr += data));
    const timer = setTimeout(() => child.kill(), 300_000);
    child.on('error', reject);
    child.on('close', (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
  });
}

export default class GrokProvider {
  id() {
    return 'grok';
  }

  async callApi(prompt) {
    writeProject();
    const args = [
      '-p', prompt, '--trust', '--output-format', 'streaming-messages-json',
      '--permission-mode', 'dontAsk', '--tools', 'search_tool,use_tool',
      '--disable-web-search', '--no-subagents', '--no-plan', '--max-turns', '15',
    ];
    if (process.env.RTM_EVAL_GROK_MODEL) args.push('-m', process.env.RTM_EVAL_GROK_MODEL);
    const { code, stdout, stderr } = await run(args);

    const toolCalls = [];
    let output = '';
    let result;
    for (const line of stdout.split('\n')) {
      let message;
      try {
        message = JSON.parse(line);
      } catch {
        continue;
      }
      if (message.type === 'result') result = message;
      if (message.type !== 'assistant') continue;
      for (const block of message.message?.content ?? []) {
        if (block.type === 'text') output = block.text;
        if (block.type !== 'tool_use' || block.name === 'search_tool') continue;
        // use_tool names an MCP tool as `server__tool`, Claude Code as `mcp__server__tool`;
        // anything else is reported under its own name.
        toolCalls.push(
          block.name === 'use_tool'
            ? { name: `mcp__${block.input?.tool_name}`, input: block.input?.tool_input ?? {} }
            : { name: block.name, input: block.input ?? {} },
        );
      }
    }
    if (!result) return { error: `grok exited with ${code} and no result: ${stderr.slice(0, 500)}` };
    return {
      output,
      cost: result.total_cost_usd,
      tokenUsage: {
        prompt: result.usage?.input_tokens,
        completion: result.usage?.output_tokens,
        cached: result.usage?.cache_read_input_tokens,
      },
      metadata: { toolCalls, terminalReason: result.subtype },
    };
  }
}
