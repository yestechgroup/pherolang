#!/usr/bin/env node
/**
 * Execute the shell commands this site prints.
 *
 * Every command shown on /install and /getting-started lives in
 * `phero-web/src/data/commands.json`; both pages render their code blocks from
 * that file, so a printed command and an executed command cannot drift apart.
 * This script is the executor: it runs each entry from the repository root with
 * the resolved binary's directory in front of `PATH`. By default that is the
 * *released* binary at `site/.bin` (fetched by
 * `scripts/fetch-pherolang.mjs`). `PHEROLANG_BIN` overrides it — the site gate
 * passes the released binary; there is no source build in the contract (#343).
 *
 * Usage:
 *   node scripts/check-commands.mjs           # run everything the gate can run
 *   node scripts/check-commands.mjs --list    # print the commands, run nothing
 *
 * Entry shape:
 *   id            stable identifier the pages look the command up by
 *   label         one line of human context
 *   run           the exact shell string printed on the page
 *   expectExit    required exit status (default 0)
 *   expectStdout  when present, a substring stdout must contain
 *   network       true = needs a download; the gate skips it
 *   cleanup       path removed after the run, relative to the repository root
 *
 * `network` is broader than the name suggests: a download block is a chain, so
 * the steps that operate on the downloaded archive (`sha256sum`, `tar`,
 * `install`, `Expand-Archive`) are marked too — they cannot run without it.
 * That chain is not untested: `scripts/fetch-pherolang.mjs` performs the same
 * download-verify-extract-install sequence against the same release and fails
 * the build if it ever stops working.
 */

import { execSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const SITE = resolve(HERE, '..');
const REPO = resolve(SITE, '..');
const BIN_DIR = join(SITE, '.bin');
const DATA = join(SITE, 'phero-web', 'src', 'data', 'commands.json');
const OVERRIDE = process.env.PHEROLANG_BIN;
const BINARY = OVERRIDE
  ? resolve(OVERRIDE)
  : join(BIN_DIR, process.platform === 'win32' ? 'pherolang.exe' : 'pherolang');
const BINARY_DIR = dirname(BINARY);
const TIMEOUT_MS = 180_000;

const listOnly = process.argv.includes('--list');

function fail(message) {
  console.error(`check-commands: ${message}`);
  process.exit(1);
}

if (!existsSync(DATA)) fail(`missing ${DATA}`);
if (!listOnly && !existsSync(BINARY)) {
  fail(
    OVERRIDE
      ? `PHEROLANG_BIN points at missing ${BINARY}`
      : `no binary at ${BINARY} — run \`node site/scripts/fetch-pherolang.mjs\` first`,
  );
}

const manifest = JSON.parse(readFileSync(DATA, 'utf8'));
if (manifest.formatVersion !== 1) {
  fail(`unsupported commands.json formatVersion ${JSON.stringify(manifest.formatVersion)}`);
}
if (!Array.isArray(manifest.commands) || manifest.commands.length === 0) {
  fail('commands.json has no commands');
}

/** `PATH` with the resolved binary first, so every printed command runs it. */
const env = {
  ...process.env,
  PATH: [BINARY_DIR, process.env.PATH ?? '']
    .filter(Boolean)
    .join(process.platform === 'win32' ? ';' : ':'),
};

/** Run one entry, reporting the failure instead of throwing it. */
function run(run) {
  try {
    const stdout = execSync(run, {
      cwd: REPO,
      env,
      encoding: 'utf8',
      timeout: TIMEOUT_MS,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    return { status: 0, stdout, stderr: '' };
  } catch (error) {
    return {
      status: typeof error.status === 'number' ? error.status : -1,
      stdout: typeof error.stdout === 'string' ? error.stdout : '',
      stderr: typeof error.stderr === 'string' ? error.stderr : String(error.message ?? error),
    };
  }
}

const idWidth = Math.max(...manifest.commands.map((entry) => entry.id.length)) + 2;
const verdictWidth = 9;
const rows = [];
let passed = 0;
let failed = 0;
let skipped = 0;

for (const entry of manifest.commands) {
  if (entry.network) {
    skipped += 1;
    rows.push({ verdict: 'skip', note: entry.run, entry });
    continue;
  }
  if (listOnly) {
    rows.push({ verdict: 'exec', note: entry.run, entry });
    continue;
  }

  const expectExit = entry.expectExit ?? 0;
  const result = run(entry.run);
  const problems = [];

  if (result.status !== expectExit) {
    problems.push(`exit ${result.status}, expected ${expectExit}`);
  }
  if (entry.expectStdout && !result.stdout.includes(entry.expectStdout)) {
    problems.push(`stdout does not contain ${JSON.stringify(entry.expectStdout)}`);
  }

  if (problems.length === 0) {
    passed += 1;
    rows.push({ verdict: 'pass', note: entry.run, entry });
  } else {
    failed += 1;
    rows.push({ verdict: 'fail', note: `${entry.run} — ${problems.join('; ')}`, entry, result });
  }
}

for (const row of rows) {
  const verdict = (row.verdict === 'skip' ? 'SKIP' : row.verdict.toUpperCase()).padEnd(verdictWidth);
  console.log(`${verdict}${row.entry.id.padEnd(idWidth)}${row.note}`);
  if (row.verdict === 'fail') {
    const stdout = row.result.stdout.trim();
    const stderr = row.result.stderr.trim();
    if (stdout) console.log(stdout.replace(/^/gm, `${' '.repeat(verdictWidth)}| `));
    if (stderr) console.log(stderr.replace(/^/gm, `${' '.repeat(verdictWidth)}| `));
  }
  if (listOnly && row.entry.label) {
    console.log(`${' '.repeat(verdictWidth + idWidth)}# ${row.entry.label}`);
  }
}

if (listOnly) {
  console.log(`\n${rows.length} commands, ${skipped} of them network-only`);
  process.exit(0);
}

// One deferred pass, so an output a later command consumes survives until the
// whole sequence has run — and a failing run still leaves no litter behind.
for (const path of new Set(manifest.commands.flatMap((entry) => entry.cleanup ?? []))) {
  rmSync(join(REPO, path), { recursive: true, force: true });
}

console.log(`\n${passed} passed, ${failed} failed, ${skipped} skipped (network)`);
if (failed > 0) process.exit(1);
