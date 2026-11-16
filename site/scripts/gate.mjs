#!/usr/bin/env node
/**
 * The local merge gate for the Phero Lang web presence.
 *
 * One entry point, eleven checks, one verdict:
 *
 *   node site/scripts/gate.mjs                 # everything, building both apps
 *   node site/scripts/gate.mjs --no-build      # reuse dist/client if it exists
 *   node site/scripts/gate.mjs --only=links    # repeatable, or --only=a,b
 *   node site/scripts/gate.mjs --list          # names and what each one proves
 *
 * Everything is deterministic and offline. No check is silently skipped: a
 * check that cannot run reports FAIL with the command that would fix it, and
 * `--only` shows the checks it excluded as SKIP(reason) rather than omitting
 * them, so a green line always means "this ran".
 *
 * The verdict is the merge decision for this PR, so the failure modes are
 * deliberately boring:
 *
 *   PASS  the check ran and every assertion in it held
 *   FAIL  it ran and something is wrong — the detail names the file and the
 *         selector/line so the owning lane can fix the page, never the gate
 *   SKIP  it was excluded by --only (the only reason a check does not run)
 *
 * Exit status: 0 only when every selected check passed; 1 otherwise; 2 for a
 * usage error.
 *
 * Reports are written to the OS temp directory (`<tmp>/pherolang-site-gate/`),
 * never into the repository: `site/` belongs to the integrator, and a gate that
 * writes into the tree it measures cannot be trusted to report on it.
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { check as a11y } from './gate/a11y.mjs';
import { check as budget } from './gate/budget.mjs';
import { check as build } from './gate/build.mjs';
import { check as contrast } from './gate/contrast.mjs';
import { check as coverage } from './gate/coverage.mjs';
import { check as freshness } from './gate/freshness.mjs';
import { check as highlighting } from './gate/highlighting.mjs';
import { check as hover } from './gate/hover.mjs';
import { check as html } from './gate/html.mjs';
import { check as links } from './gate/links.mjs';
import { REPORT_DIR } from './gate/paths.mjs';
import { check as rebrand } from './gate/rebrand.mjs';

/** Stable order: build first (it produces what the rest read), then the
 *  static checks from cheapest to most expensive, then the ones that touch the
 *  worktree or another process. */
const CHECKS = [build, links, html, contrast, a11y, budget, coverage, highlighting, hover, freshness, rebrand];

const USAGE = `usage: node site/scripts/gate.mjs [--only=<name>]... [--no-build] [--list]

  --only=<name>   run one check (repeatable, or comma-separated)
  --no-build      reuse an existing dist/client instead of building
  --list          print the checks in order and exit
  --help          this text

checks (in order): ${CHECKS.map((entry) => entry.name).join(', ')}`;

function parseArgs(argv) {
  const selected = new Set();
  const options = { only: selected, noBuild: false, list: false, help: false };

  for (const argument of argv) {
    if (argument === '--no-build') options.noBuild = true;
    else if (argument === '--list') options.list = true;
    else if (argument === '--help' || argument === '-h') options.help = true;
    else if (argument.startsWith('--only=')) {
      for (const name of argument.slice('--only='.length).split(',')) {
        const trimmed = name.trim();
        if (trimmed !== '') selected.add(trimmed);
      }
    } else {
      return { error: `unknown argument: ${argument}` };
    }
  }

  const known = new Set(CHECKS.map((entry) => entry.name));
  for (const name of selected) {
    if (!known.has(name)) return { error: `--only=${name} is not a check; known: ${[...known].join(', ')}` };
  }
  return options;
}

const seconds = (started) => (Number(process.hrtime.bigint() - started) / 1e9).toFixed(1);

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.error) {
    console.error(`gate: ${options.error}\n\n${USAGE}`);
    process.exit(2);
  }
  if (options.help) {
    console.log(USAGE);
    return 0;
  }
  if (options.list) {
    console.log(`gate: ${CHECKS.length} checks, run in this order\n`);
    for (const entry of CHECKS) console.log(`  ${entry.name.padEnd(13)}${entry.title}`);
    return 0;
  }

  const selected = options.only.size === 0 ? CHECKS : CHECKS.filter((entry) => options.only.has(entry.name));
  if (selected.length === 0) {
    console.error('gate: --only selected no checks');
    return 2;
  }

  const totalStarted = process.hrtime.bigint();
  const startedAt = new Date().toISOString();
  const header = [
    '# Phero Lang site gate',
    '',
    `- started: ${startedAt}`,
    `- node: ${process.version}`,
    `- cwd: ${process.cwd()}`,
    `- mode: ${options.noBuild ? 'no-build (reusing dist/client)' : 'build both apps'}`,
    `- checks: ${selected.map((entry) => entry.name).join(', ')}`,
    '',
  ];
  const body = [];

  console.log(`gate: ${selected.length} of ${CHECKS.length} checks, node ${process.version}`);
  if (options.noBuild) {
    console.log('gate: --no-build, reusing existing dist/client trees\n');
  } else if (selected.some((entry) => entry.name === 'build')) {
    console.log('gate: building both apps first\n');
  } else {
    console.log('gate: build not selected — dist/client must already exist\n');
  }

  const results = [];
  for (const entry of CHECKS) {
    if (!selected.includes(entry)) {
      const reason = `not selected (--only=${[...options.only].join(',')})`;
      console.log(`SKIP ${entry.name}  (${reason})`);
      body.push(`## SKIP ${entry.name}`, '', `reason: ${reason}`, '');
      continue;
    }

    const checkStarted = process.hrtime.bigint();
    let outcome;
    try {
      outcome = await entry.run({ noBuild: options.noBuild });
    } catch (error) {
      outcome = {
        status: 'fail',
        lines: [
          `FAIL the check itself threw: ${String(error?.message ?? error).split('\n')[0]}`,
          ...(error?.stack ? String(error.stack).split('\n').slice(1, 6).map((line) => `     ${line.trim()}`) : []),
        ],
      };
    }
    const elapsed = seconds(checkStarted);

    const verdict = outcome.status === 'pass' ? 'PASS' : 'FAIL';
    console.log(`${verdict} ${entry.name}  (${elapsed}s)`);
    for (const line of outcome.lines ?? []) console.log(`  ${line}`);

    results.push({ name: entry.name, status: outcome.status, elapsed });
    body.push(
      `## ${verdict} ${entry.name} (${elapsed}s)`,
      '',
      ...(outcome.lines ?? []).map((line) => line.trimEnd()),
      '',
    );
  }

  const failed = results.filter((result) => result.status !== 'pass');
  const elapsedTotal = seconds(totalStarted);

  const summary = results
    .map((result) => `${result.status === 'pass' ? 'ok' : 'FAIL'} ${result.name} (${result.elapsed}s)`)
    .join(', ');

  console.log('');
  console.log(`gate: ${results.length - failed.length}/${results.length} passed in ${elapsedTotal}s`);
  console.log(`gate: ${summary}`);

  if (failed.length > 0) {
    console.log('');
    console.log(`gate: FAIL — ${failed.map((result) => result.name).join(', ')} must be fixed before merge`);
  } else {
    console.log('gate: PASS — every selected check holds');
  }

  const report = [
    ...header,
    `result: ${failed.length === 0 ? 'PASS' : 'FAIL'}`,
    `duration: ${elapsedTotal}s`,
    '',
    ...body,
    '## Summary',
    '',
    `- ${results.length - failed.length}/${results.length} passed in ${elapsedTotal}s`,
    `- ${summary}`,
    `- verdict: ${failed.length === 0 ? 'PASS' : 'FAIL'}`,
  ];

  mkdirSync(REPORT_DIR, { recursive: true });
  const stamp = startedAt.replace(/[:.]/g, '-');
  const reportPath = join(REPORT_DIR, `gate-${stamp}.md`);
  writeFileSync(reportPath, `${report.join('\n')}\n`, 'utf8');
  writeFileSync(join(REPORT_DIR, 'latest.md'), `${report.join('\n')}\n`, 'utf8');

  console.log('');
  console.log(`gate: report written outside the repository: ${reportPath}`);

  return failed.length === 0 ? 0 : 1;
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error) => {
    console.error(`gate: unrecoverable — ${error?.stack ?? error}`);
    process.exitCode = 1;
  });
