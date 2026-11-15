/**
 * Check 8 — freshness: the committed model data is what the pinned released
 * binary produces.
 *
 * `site/data/**` is generated (see `site/data/CONTRACT.md`) and committed. The
 * only thing that makes the site's claims about the toolchain honest is that
 * regenerating with the same inputs and the same released binary is a no-op.
 * So this check regenerates and then asks git.
 *
 * It restores nothing on failure. A dirty tree *is* the finding: reverting it
 * would delete the evidence the integrator needs, and `git diff` is already the
 * report.
 *
 * Never a workspace `cargo build`: the binary is the sha256-verified released
 * asset at `site/.bin/pherolang` (`scripts/fetch-pherolang.mjs`), matching what
 * CI downloads.
 */

import { join } from 'node:path';

import { PHEROLANG_BIN, REPO, SITE } from './paths.mjs';
import { isFile, plural, sh, tail } from './util.mjs';

const NAME = 'freshness';

const relativeData = join('site', 'data');

/** `git status --porcelain -- site/data` as a list of lines. */
function dataStatus() {
  const result = sh('git', ['status', '--porcelain', '--', relativeData], { cwd: REPO, timeout: 60_000 });
  if (result.status !== 0) return null;
  return result.stdout.trim() === '' ? [] : result.stdout.trim().split('\n');
}

export const check = {
  name: NAME,
  title: 'node scripts/sync-model.mjs is a no-op against the committed site/data',

  run() {
    const lines = [];

    if (!isFile(PHEROLANG_BIN)) {
      lines.push(`FAIL no released compiler at ${PHEROLANG_BIN}`);
      lines.push('     fetch it with: node scripts/fetch-pherolang.mjs   (the sha256-verified release asset CI uses)');
      return { status: 'fail', lines };
    }

    const before = dataStatus();
    if (before === null) {
      lines.push('FAIL `git status` failed — freshness needs a git worktree');
      return { status: 'fail', lines };
    }
    if (before.length > 0) {
      lines.push(
        `note: site/data was already dirty before this check ran (${plural(before.length, 'path')}) — reported as-is, not attributed to the regeneration`,
      );
      for (const line of before.slice(0, 10)) lines.push(`     before ${line}`);
    }

    const started = process.hrtime.bigint();
    const sync = sh('node', [join(SITE, 'scripts', 'sync-model.mjs')], {
      cwd: SITE,
      timeout: 900_000,
    });
    const seconds = Number(process.hrtime.bigint() - started) / 1e9;

    if (sync.status !== 0) {
      lines.push(`FAIL node scripts/sync-model.mjs exited ${sync.status} after ${seconds.toFixed(1)}s`);
      if (sync.stdout.trim()) lines.push(tail(sync.stdout, 20).replace(/^/gm, '     | '));
      if (sync.stderr.trim()) lines.push(tail(sync.stderr, 20).replace(/^/gm, '     | '));
      return { status: 'fail', lines };
    }

    // The plan's sentence is `git diff --exit-code -- site/data`; that exit code
    // is the verdict. `git status --porcelain` runs alongside it because a
    // regenerated *new* file is untracked, and `git diff` does not see those.
    const diff = sh('git', ['diff', '--exit-code', '--', relativeData], { cwd: REPO, timeout: 60_000 });
    const stat = sh('git', ['diff', '--stat', '--', relativeData], { cwd: REPO, timeout: 60_000 });
    const after = dataStatus();
    if (after === null) {
      lines.push('FAIL `git status` failed after regeneration');
      return { status: 'fail', lines };
    }

    const modified = diff.status !== 0;
    const untracked = after.filter((line) => line.startsWith('??'));
    if (!modified && untracked.length === 0) {
      lines.push(`ok   regenerated site/data in ${seconds.toFixed(1)}s and git reports no change (0 paths)`);
      return { status: 'pass', lines };
    }

    lines.push(
      `FAIL regeneration changed the committed model data (${plural(after.length, 'path')}) — ` +
        `git diff --exit-code -- site/data exited ${diff.status}`,
    );
    for (const line of after) lines.push(`     ${line}`);
    if (stat.stdout.trim()) lines.push(stat.stdout.trimEnd().replace(/^/gm, '     | '));
    lines.push(
      '     the working tree is left as found: review the diff, then either commit the regenerated data or fix the source',
    );
    return { status: 'fail', lines };
  },
};
