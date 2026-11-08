// Before each test: a fresh scratch state from the test's vars, in $RTM_EVAL_DIR (made and
// removed by scripts/mcp-eval.sh). Tests run one at a time, so they can share the directory.
//
//   todo/home.txt, todo/work.txt   the files, from vars.home and vars.work
//   start/                         a copy of them, which the checks compare with
//   home/                          HOME and XDG_CONFIG_HOME of the server: rtmapp's config there
//                                  lists both files, home.txt first (the default)
//   todo/.git                      with vars.git: the files committed in a repository whose
//                                  remote has gone away, so every pull and push fails

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const dir = process.env.RTM_EVAL_DIR;

export function hook(name, context) {
  if (name === 'beforeEach') setUp(context.test.vars ?? {});
}

function setUp(vars) {
  // The directory is emptied below: only ever the script's own scratch directory.
  if (!dir || !path.basename(dir).startsWith('rtm-mcp-eval.'))
    throw new Error('RTM_EVAL_DIR is not a scratch directory of scripts/mcp-eval.sh');
  fs.rmSync(dir, { recursive: true, force: true });
  const todo = path.join(dir, 'todo');
  const home = path.join(todo, 'home.txt');
  const work = path.join(todo, 'work.txt');
  const start = path.join(dir, 'start');
  for (const files of [todo, start]) {
    fs.mkdirSync(files, { recursive: true });
    fs.writeFileSync(path.join(files, 'home.txt'), vars.home ?? '');
    fs.writeFileSync(path.join(files, 'work.txt'), vars.work ?? '');
  }

  const config = [
    `file_name = ${JSON.stringify(home)}`,
    'sync = true',
    '',
    '[[files]]',
    `path = ${JSON.stringify(home)}`,
    'name = "home"',
    '',
    '[[files]]',
    `path = ${JSON.stringify(work)}`,
    'name = "work"',
    '',
  ].join('\n');
  for (const configDir of [
    path.join(dir, 'home', '.config', 'rtm'),
    path.join(dir, 'home', 'Library', 'Application Support', 'rtm'),
  ]) {
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(path.join(configDir, 'config.toml'), config);
  }

  if (vars.git) {
    const hooks = path.join(dir, 'no-hooks');
    fs.mkdirSync(hooks);
    git(todo, 'init', '--quiet');
    // As in the rtmmcp git tests: a local identity, no signing, no hooks, no rerere.
    for (const [key, value] of [
      ['user.name', 'rtm eval'],
      ['user.email', 'rtm@example.com'],
      ['commit.gpgsign', 'false'],
      ['core.hooksPath', hooks],
      ['rerere.enabled', 'false'],
    ]) {
      git(todo, 'config', key, value);
    }
    git(todo, 'add', 'home.txt', 'work.txt');
    git(todo, 'commit', '--quiet', '-m', 'start');
    // The server syncs only a branch with an upstream: push to a remote once, then remove it.
    const remote = path.join(dir, 'remote.git');
    git(dir, 'init', '--quiet', '--bare', remote);
    git(todo, 'remote', 'add', 'origin', remote);
    git(todo, 'push', '--quiet', '--set-upstream', 'origin', 'HEAD');
    fs.rmSync(remote, { recursive: true });
  }
}

export function git(cwd, ...args) {
  return execFileSync('git', args, {
    cwd,
    encoding: 'utf8',
    env: { ...process.env, HOME: path.join(dir, 'home'), GIT_CONFIG_NOSYSTEM: '1' },
  });
}
