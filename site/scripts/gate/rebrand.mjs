/**
 * Check 9 — the repository's own rebrand gate.
 *
 * The pre-0.3 product name is gated by `scripts/check-rebrand.sh` (CI step 2/7,
 * epic pherolang#270): it fails on a revived old-name token anywhere in the
 * tracked tree outside its documented allowlist. This site is part of the
 * tracked tree, so the merge gate runs that script rather than re-implementing
 * its allowlist — one list, one owner, and `site/` cannot drift into a private
 * copy of the rule.
 *
 * Run from the repository root, unfiltered. A non-zero exit is the finding.
 */

import { REPO } from './paths.mjs';
import { isFile, sh, tail } from './util.mjs';
import { join } from 'node:path';

const NAME = 'rebrand';

const SCRIPT = join(REPO, 'scripts', 'check-rebrand.sh');

export const check = {
  name: NAME,
  title: 'scripts/check-rebrand.sh passes over the tracked tree',

  run() {
    const lines = [];

    if (!isFile(SCRIPT)) {
      lines.push(`FAIL ${SCRIPT} is missing — the rebrand gate cannot run`);
      return { status: 'fail', lines };
    }

    const started = process.hrtime.bigint();
    const result = sh('bash', [SCRIPT], { cwd: REPO, timeout: 300_000 });
    const seconds = Number(process.hrtime.bigint() - started) / 1e9;

    const output = [result.stdout, result.stderr].filter((part) => part.trim() !== '').join('\n');

    if (result.status !== 0) {
      lines.push(`FAIL bash scripts/check-rebrand.sh exited ${result.status} after ${seconds.toFixed(1)}s`);
      if (output.trim()) lines.push(tail(output, 30).replace(/^/gm, '     | '));
      return { status: 'fail', lines };
    }

    const summary = output.trim() === '' ? '' : tail(output, 3).replace(/^/gm, '     | ');
    lines.push(`ok   bash scripts/check-rebrand.sh exited 0 in ${seconds.toFixed(1)}s`);
    if (summary) lines.push(summary);
    return { status: 'pass', lines };
  },
};
