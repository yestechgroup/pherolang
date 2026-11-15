/**
 * Check 1 — both apps build, and both produce the pages this PR claims.
 *
 * The build is the only check that mutates the working tree (it writes
 * `dist/`, which is gitignored). `--no-build` reuses whatever is on disk; it
 * never falls back to a build, so "pass with stale output" is a decision the
 * caller makes explicitly, and the check says which mode it used.
 */

import { join } from 'node:path';

import { APPS } from './paths.mjs';
import { isDir, isFile, plural, sh, tail, walk } from './util.mjs';

const NAME = 'build';

export const check = {
  name: NAME,
  title: 'both apps build and emit their promised page counts',

  run(ctx) {
    const lines = [];
    let failed = false;
    /** Apps whose build (or reuse) already failed, so the count pass stays quiet. */
    const reported = new Set();

    for (const app of Object.values(APPS)) {
      if (ctx.noBuild) {
        if (!isDir(app.dist)) {
          failed = true;
          reported.add(app.label);
          lines.push(
            `FAIL ${app.label}: --no-build was passed but ${app.dist} does not exist — drop --no-build`,
          );
        } else {
          lines.push(`ok   ${app.label}: --no-build, reusing ${app.dist}`);
        }
        continue;
      }

      const started = process.hrtime.bigint();
      const result = sh('pnpm', ['-C', app.packageDir, 'build'], {
        cwd: app.packageDir,
        timeout: 900_000,
      });
      const seconds = Number(process.hrtime.bigint() - started) / 1e9;

      if (result.status !== 0) {
        failed = true;
        reported.add(app.label);
        lines.push(
          `FAIL ${app.label}: \`${result.command}\` exited ${result.status} after ${seconds.toFixed(1)}s`,
        );
        if (result.signal) lines.push(`     killed by signal ${result.signal}`);
        if (result.stderr.trim()) lines.push(tail(result.stderr, 40).replace(/^/gm, '     | '));
        if (result.stdout.trim()) lines.push(tail(result.stdout, 40).replace(/^/gm, '     | '));
        continue;
      }

      lines.push(`ok   ${app.label}: built in ${seconds.toFixed(1)}s`);
    }

    for (const app of Object.values(APPS)) {
      if (!isDir(app.dist)) {
        if (!reported.has(app.label)) {
          failed = true;
          lines.push(`FAIL ${app.label}: the build reported success but ${app.dist} does not exist`);
        }
        continue;
      }

      const pages = walk(app.dist, (file) => file.endsWith('.html'));
      if (pages.length < app.minPages) {
        failed = true;
        lines.push(
          `FAIL ${app.label}: ${plural(pages.length, 'html file')} under dist/client, expected at least ${app.minPages}`,
        );
      } else {
        lines.push(`ok   ${app.label}: ${pages.length} html files (floor ${app.minPages})`);
      }

      const entry = join(app.dist, 'index.html');
      if (!isFile(entry)) {
        failed = true;
        lines.push(`FAIL ${app.label}: dist/client/index.html is missing — the origin has no landing page`);
      }
    }

    return { status: failed ? 'fail' : 'pass', lines };
  },
};
