/**
 * Process, filesystem and formatting helpers shared by the checks.
 *
 * Determinism rule: every directory listing is sorted, every timing is taken
 * once, and nothing here reads `Date` for anything that lands in a verdict.
 */

import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

/**
 * Run a child process and return its outcome instead of throwing.
 *
 * `stdio` is piped so a failing command can be reported with its own output;
 * checks that print their own progress pass `echo: true` to stream.
 */
export function sh(command, args, options = {}) {
  const { cwd, env, timeout = 600_000, echo = false } = options;
  const result = spawnSync(command, args, {
    cwd,
    env: env ?? process.env,
    encoding: 'utf8',
    timeout,
    maxBuffer: 64 * 1024 * 1024,
    stdio: echo ? ['ignore', 'inherit', 'inherit'] : ['ignore', 'pipe', 'pipe'],
  });
  return {
    command: [command, ...args].join(' '),
    status: typeof result.status === 'number' ? result.status : -1,
    signal: result.signal ?? null,
    stdout: typeof result.stdout === 'string' ? result.stdout : '',
    stderr:
      typeof result.stderr === 'string'
        ? result.stderr
        : (result.error?.message ?? String(result.error ?? '')),
  };
}

/** Recursively list files under `dir` as base-relative POSIX paths, sorted. */
export function walk(dir, predicate = () => true) {
  const out = [];
  const visit = (current) => {
    let entries;
    try {
      entries = readdirSync(current, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) visit(full);
      else if (entry.isFile() && predicate(full)) out.push(relative(dir, full).split(sep).join('/'));
    }
  };
  visit(dir);
  return out;
}

export function isDir(path) {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

export function isFile(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

export function readText(path) {
  return readFileSync(path, 'utf8');
}

export function readJson(path) {
  return JSON.parse(readText(path));
}

/** Byte count rendered for humans, fixed at one decimal so runs compare. */
export function bytes(value) {
  return `${(value / 1024).toFixed(2)} KB`;
}

/** "1 page" / "43 pages" — keeps every count line readable without plurals logic. */
export function plural(count, one, many = `${one}s`) {
  return `${count} ${count === 1 ? one : many}`;
}

/** Last `n` lines of captured output, for a failure report. */
export function tail(text, n = 30) {
  const lines = text.replace(/\s+$/, '').split('\n');
  return lines.slice(Math.max(0, lines.length - n)).join('\n');
}
