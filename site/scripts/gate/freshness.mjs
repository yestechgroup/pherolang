/**
 * Check 10 — freshness: the committed `site/data` is what the pinned/released
 * toolchain produces.
 *
 * `site/data/**` is generated (see `site/data/CONTRACT.md`) and committed. The
 * only thing that makes the site's claims about the toolchain honest is that
 * regenerating with the same inputs and the same binary is a no-op. Two
 * scripts own that data:
 *
 *   - `node scripts/sync-model.mjs` regenerates the model artifacts;
 *   - `node scripts/sync-highlight.mjs` regenerates the compiler-backed
 *     highlight artifact.
 *
 * Each uses the resolved site binary: the sha256-verified release when it can
 * check the current examples and provides the capabilities the script needs,
 * otherwise a source build of this checkout. The release v0.3.0 predates both
 * the DD9 examples and `pherolang highlight`, so the fallback is the documented
 * temporary exception — never released-binary freshness (`docs/HIGHLIGHTING.md`).
 *
 * So this check runs both and then asks git.
 *
 * It restores nothing on failure. A dirty tree *is* the finding: reverting it
 * would delete the evidence the integrator needs, and `git diff` is already the
 * report.
 *
 * A source build is only used when the release cannot do the job, and the
 * report says so.
 */

import { join } from 'node:path';

import { resolveSiteBinary } from '../highlighting/runner.mjs';
import { REPO, SITE } from './paths.mjs';
import { plural, sh, tail } from './util.mjs';

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
  title: 'sync-model.mjs and sync-highlight.mjs are no-ops against the committed site/data',

  run() {
    const lines = [];

    let modelBinary;
    let highlightBinary;
    try {
      modelBinary = resolveSiteBinary(REPO, SITE, {
        requireHighlight: false,
        log: (message) => lines.push(`     note: ${message}`),
      });
      highlightBinary = resolveSiteBinary(REPO, SITE, {
        requireHighlight: true,
        log: (message) => lines.push(`     note: ${message}`),
      });
    } catch (error) {
      lines.push(`FAIL no usable pherolang binary: ${error.message}`);
      lines.push('     fetch the release with: node scripts/fetch-pherolang.mjs, or set PHEROLANG_BIN');
      return { status: 'fail', lines };
    }

    lines.push(
      `     model binary: ${modelBinary.kind} (${modelBinary.binary})`,
      `     highlight binary: ${highlightBinary.kind} (${highlightBinary.binary})`,
    );

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

    const runScript = (script, binary) => {
      const started = process.hrtime.bigint();
      const result = sh('node', [join(SITE, 'scripts', script)], {
        cwd: SITE,
        timeout: 900_000,
        env: { ...process.env, PHEROLANG_BIN: binary.binary },
      });
      return { ...result, elapsed: Number(process.hrtime.bigint() - started) / 1e9 };
    };

    const scripts = [
      { script: 'sync-model.mjs', binary: modelBinary },
      { script: 'sync-highlight.mjs', binary: highlightBinary },
    ].map(({ script, binary }) => ({
      script,
      ...runScript(script, binary),
    }));
    const seconds = scripts.reduce((total, result) => total + result.elapsed, 0);

    for (const result of scripts) {
      if (result.status !== 0) {
        lines.push(
          `FAIL node scripts/${result.script} exited ${result.status} after ${result.elapsed.toFixed(1)}s`,
        );
        if (result.stdout.trim()) lines.push(tail(result.stdout, 20).replace(/^/gm, '     | '));
        if (result.stderr.trim()) lines.push(tail(result.stderr, 20).replace(/^/gm, '     | '));
        return { status: 'fail', lines };
      }
    }

    if (modelBinary.kind === 'source-build') {
      lines.push(
        'note: model data was regenerated from a source build because the latest release predates the current examples',
      );
    }
    if (highlightBinary.kind === 'source-build') {
      lines.push(
        'note: the highlight artifact was generated from a source build because the latest release predates `pherolang highlight`',
      );
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
      `FAIL regeneration changed the committed site data (${plural(after.length, 'path')}) — ` +
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
