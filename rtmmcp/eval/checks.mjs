// The checks of promptfooconfig.yaml's tests: the files in $RTM_EVAL_DIR after the run, and the
// tool calls the client reported. promptfoo runs them right after each test's run, before the
// next test's hook resets the files.

import fs from 'node:fs';
import path from 'node:path';

import { git } from './hooks.mjs';

const dir = process.env.RTM_EVAL_DIR;

const lines = (file, base = 'todo') =>
  fs.readFileSync(path.join(dir, base, file), 'utf8').split('\n').filter(Boolean);

// Local date, as the server writes completion dates.
function today() {
  const now = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

const isDone = (line) => line.startsWith(`x ${today()} `);

// Runs a check: `test` gets the tool calls (rtm's tool names without the client's prefix) and
// returns its failures. Any tool other than rtm's fails the test too: the client must not reach
// the files around the server.
function check(context, test) {
  const all = context.providerResponse?.metadata?.toolCalls ?? [];
  const failures = all
    .filter((call) => !call.name.startsWith('mcp__rtm__'))
    .map((call) => `used ${call.name}`);
  const calls = all
    .filter((call) => call.name.startsWith('mcp__rtm__'))
    .map((call) => ({ ...call, name: call.name.slice('mcp__rtm__'.length), input: call.input ?? {} }));
  failures.push(...test(calls));
  const used = calls.map((call) => call.name).join(', ') || 'no tools';
  return {
    pass: failures.length === 0,
    score: failures.length === 0 ? 1 : 0,
    reason: failures.length === 0 ? `ok (${used})` : `${failures.join('; ')} (${used})`,
  };
}

const named = (calls, name) => calls.filter((call) => call.name === name);

function only(calls, allowed) {
  const others = calls.filter((call) => !allowed.includes(call.name) && !call.name.startsWith('list_'));
  return others.map((call) => `unexpected ${call.name}`);
}

// Every line of `file` but those matching `changed` is as the test started.
function othersUnchanged(file, changed) {
  const before = lines(file, 'start').filter((line) => !changed.test(line));
  const after = lines(file).filter((line) => !changed.test(line));
  return JSON.stringify(before) === JSON.stringify(after)
    ? []
    : [`other lines of ${file} changed: ${JSON.stringify(before)} became ${JSON.stringify(after)}`];
}

export function completeMilk(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['complete_item']);
    const completes = named(calls, 'complete_item');
    if (completes.length !== 1) failures.push(`${completes.length} complete_item calls`);
    else if (completes[0].input.number !== 1 || completes[0].input.text !== '2026-11-01 Buy milk +groceries')
      failures.push(`complete_item with ${JSON.stringify(completes[0].input)}`);
    if (!lines('home.txt').some((line) => isDone(line) && line.includes('Buy milk'))) failures.push('milk not completed');
    failures.push(...othersUnchanged('home.txt', /Buy milk/));
    return failures;
  });
}

export function staleLine(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['complete_item']);
    // The plumber item may be completed (after listing again) or left open (the model asked);
    // nothing else may change.
    failures.push(...othersUnchanged('home.txt', /plumber/));
    const plumber = lines('home.txt').filter((line) => line.includes('plumber'));
    if (plumber.length !== 1) failures.push(`${plumber.length} plumber lines`);
    return failures;
  });
}

export function doneWithRent(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['complete_item']);
    if (!lines('home.txt').some((line) => isDone(line) && line.includes('Pay rent'))) failures.push('rent not completed');
    failures.push(...othersUnchanged('home.txt', /Pay rent/));
    return failures;
  });
}

export function declinedDelete(_output, context) {
  return check(context, (calls) => {
    // delete_item is not allowed, so the client declines it; nothing may stand in for it.
    const failures = only(calls, ['delete_item']);
    failures.push(...othersUnchanged('home.txt', /^$/));
    return failures;
  });
}

export function notSynced(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['add_item']);
    const adds = named(calls, 'add_item');
    if (adds.length !== 1) failures.push(`${adds.length} add_item calls`);
    const bread = lines('home.txt').filter((line) => /bread/i.test(line));
    if (bread.length !== 1) failures.push(`${bread.length} bread lines`);
    failures.push(...othersUnchanged('home.txt', /bread/i));
    const commits = git(path.join(dir, 'todo'), 'rev-list', '--count', 'HEAD').trim();
    if (commits !== '2') failures.push(`${commits} commits, expected the start and one rtm commit`);
    return failures;
  });
}

export function editReport(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['edit_item']);
    const edits = named(calls, 'edit_item');
    if (edits.length !== 1) failures.push(`${edits.length} edit_item calls`);
    const report = lines('home.txt').filter((line) => line.includes('quarterly report'));
    if (report.length !== 1) failures.push(`${report.length} report lines`);
    else if (!/^\(A\) 2026-11-05 .*due:2026-11-20/.test(report[0])) failures.push(`report reads "${report[0]}"`);
    failures.push(...othersUnchanged('home.txt', /quarterly report/));
    return failures;
  });
}

export function workFile(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['add_item']);
    const adds = named(calls, 'add_item');
    if (adds.length !== 1) failures.push(`${adds.length} add_item calls`);
    else if (!/work/.test(adds[0].input.file ?? '')) failures.push(`add_item to file ${JSON.stringify(adds[0].input.file)}`);
    if (lines('work.txt').filter((line) => /call mom/i.test(line)).length !== 1) failures.push('no call mom line in work.txt');
    failures.push(...othersUnchanged('work.txt', /call mom/i));
    failures.push(...othersUnchanged('home.txt', /^$/));
    return failures;
  });
}

export function recurring(_output, context) {
  return check(context, (calls) => {
    const failures = only(calls, ['complete_item']);
    const plants = lines('home.txt').filter((line) => line.includes('Water the plants'));
    const done = plants.filter(isDone);
    const open = plants.filter((line) => !line.startsWith('x '));
    if (done.length !== 1) failures.push(`${done.length} completed plant lines`);
    if (open.length !== 1) failures.push(`${open.length} open plant lines`);
    // ISO dates compare as strings: the next occurrence is due after the completed one.
    else if (!((open[0].match(/due:(\S+)/)?.[1] ?? '') > '2026-11-08')) failures.push(`next occurrence reads "${open[0]}"`);
    failures.push(...othersUnchanged('home.txt', /Water the plants/));
    return failures;
  });
}
