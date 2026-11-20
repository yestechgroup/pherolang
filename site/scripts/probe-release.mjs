/**
 * Probe: can the released `pherolang` binary compile the models on main?
 *
 * `freshness` and `drift` (`.github/workflows/site.yml`) regenerate the
 * committed `site/data` with the released binary alone — there is no
 * source-build fallback. When the release predates the DSL on main, the jobs
 * must not fall back and must not go generically red either: they emit the
 * **named skip** (release tag + the failing file) and skip the regeneration.
 * This module owns that decision, so the workflow step stays a one-liner and
 * the behaviour is unit-tested (`site/scripts/test/probe-release.test.mjs`).
 *
 * The named skip, exactly:
 *
 *     the latest release (vX.Y.Z) predates the DSL on main:
 *     examples/support/support.ddd uses syntax it does not parse —
 *     regenerate after the next release
 *
 * CLI contract:
 *
 *     node site/scripts/probe-release.mjs \
 *       [--binary <pherolang>] [--examples <dir>] [--tag vX.Y.Z] \
 *       [--skip-file <path>]
 *
 * Exit codes: `0` compatible (the release compiles `examples/`), `2`
 * incompatible (the named skip — the message is printed and, when given,
 * written to `--skip-file`), `1` infrastructure failure (missing binary or
 * examples directory — never a named skip). The JSON verdict goes to stdout
 * in both decided cases.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { extname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

/** Every surface `pherolang check` accepts — the probe walks these extensions. */
const SURFACE_EXTENSIONS = new Set([
  '.mox',
  '.actor',
  '.ddd',
  '.evt',
  '.dmn',
  '.deploy',
  '.uc',
  '.cmmn',
  '.ifml',
  '.intent',
]);

/** The named skip's exact shape — the issue's wording, tag + failing file. */
export function skipMessage({ tag, failingFile }) {
  return (
    `the latest release (${tag}) predates the DSL on main: ` +
    `${failingFile} uses syntax it does not parse — regenerate after the next release`
  );
}

/** Every model file under `dir` (recursive), sorted for a stable verdict. */
export function listModelFiles(dir) {
  const files = [];
  const walk = (entry) => {
    const stat = statSync(entry);
    if (stat.isDirectory()) {
      for (const name of readdirSync(entry).sort()) walk(join(entry, name));
    } else if (SURFACE_EXTENSIONS.has(extname(entry))) {
      files.push(entry);
    }
  };
  walk(dir);
  return files;
}

/** Run one `check`; returns the spawn result (status/error never thrown). */
function runCheck(binary, target) {
  return spawnSync(binary, ['check', target], { encoding: 'utf8' });
}

/**
 * The verdict. Returns `{ compatible: true }` when the release compiles the
 * directory, or `{ compatible: false, failingFile }` naming the first file
 * the release cannot parse **or silently skips** (a release predating a
 * surface compiles nothing for it in directory mode — no `OK <path>` line —
 * so coverage of the check's own receipts is part of the verdict). Throws
 * (infrastructure failure) when the binary
 * cannot run at all or the examples directory is missing — those are never
 * the named skip, because "no binary" is a broken job, not an old release.
 */
export function probeRelease({ binary, examplesDir }) {
  if (!binary) throw new Error('probe needs a binary path (--binary)');
  if (!existsSync(binary)) throw new Error(`binary does not exist: ${binary}`);
  if (!existsSync(examplesDir)) throw new Error(`examples directory does not exist: ${examplesDir}`);

  const whole = runCheck(binary, examplesDir);
  if (whole.error) throw new Error(`cannot execute ${binary}: ${whole.error.message}`);
  if (whole.status === 0) {
    // A release that predates a surface **silently skips** its files in
    // directory mode (the unknown-extension default compiles nothing and
    // prints nothing) — the `OK <path>` line is the only receipt that the
    // release compiled a file. Every model file must be claimed by one;
    // an unclaimed file is exactly the named skip: the release predates
    // that surface.
    const files = listModelFiles(examplesDir);
    const okPaths = new Set(
      (whole.stdout ?? '')
        .split('\n')
        .filter((line) => line.startsWith('OK '))
        .map((line) => line.slice(3).trim()),
    );
    const unclaimed = files.find((file) => {
      const suffix = file.slice(examplesDir.length + 1);
      for (const ok of okPaths) {
        if (ok === suffix || ok.endsWith(`/${suffix}`)) return false;
      }
      return true;
    });
    if (unclaimed) return { compatible: false, failingFile: unclaimed };
    return { compatible: true };
  }

  // The directory failed. Pin the first file the release cannot parse by
  // checking each model file on its own — deterministic (sorted walk), and
  // the named skip quotes a file, not a directory.
  for (const file of listModelFiles(examplesDir)) {
    const single = runCheck(binary, file);
    if (single.error) throw new Error(`cannot execute ${binary}: ${single.error.message}`);
    if (single.status !== 0) return { compatible: false, failingFile: file };
  }
  // Every file passes alone but the directory failed — a cross-file failure
  // (import resolution, duplicate declarations). Still incompatible; the
  // directory is the honest thing to name.
  return { compatible: false, failingFile: examplesDir };
}

/**
 * Run the probe and describe the outcome for a workflow step: `{ exitCode,
 * verdict }` where `exitCode` is 0/2 per the CLI contract (1 on
 * infrastructure failure). Writes the named skip to `skipFile` when given
 * and the release is incompatible.
 */
export function probeForCi({ binary, examplesDir, tag, skipFile } = {}) {
  let verdict;
  try {
    verdict = probeRelease({ binary, examplesDir });
  } catch (error) {
    process.stderr.write(`probe-release: ${error.message}\n`);
    return { exitCode: 1, verdict: null };
  }

  if (verdict.compatible) {
    process.stdout.write(`${JSON.stringify({ compatible: true, tag })}\n`);
    return { exitCode: 0, verdict: { compatible: true, tag } };
  }

  const message = skipMessage({ tag, failingFile: verdict.failingFile });
  process.stdout.write(`${JSON.stringify({ compatible: false, tag, failingFile: verdict.failingFile })}\n`);
  process.stderr.write(`${message}\n`);
  if (skipFile) writeFileSync(skipFile, `${message}\n`);
  return { exitCode: 2, verdict: { compatible: false, tag, failingFile: verdict.failingFile, message } };
}

function main(argv) {
  const option = (name) => {
    const index = argv.indexOf(name);
    return index >= 0 ? argv[index + 1] : undefined;
  };
  const result = probeForCi({
    binary: option('--binary') ?? process.env.PHEROLANG_BIN ?? 'pherolang',
    examplesDir: option('--examples') ?? 'examples',
    tag: option('--tag') ?? 'unknown',
    skipFile: option('--skip-file'),
  });
  process.exit(result.exitCode);
}

// `node --test` imports this module for the exported functions; only run the
// CLI when executed directly.
const invokedDirectly =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href;
if (invokedDirectly) {
  main(process.argv.slice(2));
}
