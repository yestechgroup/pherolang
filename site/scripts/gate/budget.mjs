/**
 * Check 6 — the client-JS budget.
 *
 * The marketing site is static-first: every page renders complete HTML and the
 * only shipped script is the graph island (the single-page budget does not
 * count Starlight's own search UI, which belongs to the docs origin). This
 * check sums the gzipped bytes of every distinct `.js` file the built marketing
 * pages reference, so a second island, an analytics bundle or a "@astrojs
 * prefetch" runtime turns a passing build into a failing one instead of a
 * surprise on the next Lighthouse run.
 *
 * Gzip is measured at level 9 over the exact bytes the origin serves. Inline
 * scripts are counted and reported but not budgeted — the budget's sentence is
 * "one 1.8 KB island", i.e. files.
 */

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

import { APPS, JS_BUDGET_BYTES } from './paths.mjs';
import { scripts } from './html-scan.mjs';
import { bytes, isFile, plural, readText, walk } from './util.mjs';

const NAME = 'budget';

const gzipped = (buffer) => gzipSync(buffer, { level: 9 }).length;

export const check = {
  name: NAME,
  title: `marketing client JS stays under ${JS_BUDGET_BYTES / 1024} KB gzipped`,

  run() {
    const lines = [];
    const problems = [];
    const referenced = new Map();
    const inline = [];
    let pages = 0;

    for (const rel of walk(APPS.web.dist, (file) => file.endsWith('.html'))) {
      const html = readText(join(APPS.web.dist, rel));
      pages += 1;
      for (const script of scripts(html)) {
        if (typeof script.src === 'string' && script.src !== '') {
          if (!referenced.has(script.src)) referenced.set(script.src, new Set());
          referenced.get(script.src).add(rel);
        } else if ((script.type ?? '').includes('javascript') || script.type === undefined) {
          // Inline classic/module scripts: reported, not budgeted.
          inline.push(rel);
        }
      }
    }

    let total = 0;
    let raw = 0;
    for (const [src, from] of [...referenced].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
      if (!/^\/.*\.js$/i.test(src)) {
        problems.push(`${src} is a script source that is not a root-relative .js path`);
        continue;
      }
      const file = join(APPS.web.dist, src.replace(/^\/+/, ''));
      if (!isFile(file)) {
        problems.push(`${src} is referenced by ${plural(from.size, 'page')} but not present in dist/client`);
        continue;
      }
      const buffer = readFileSync(file);
      const rawBytes = buffer.length;
      const gzipBytes = gzipped(buffer);
      total += gzipBytes;
      raw += rawBytes;
      lines.push(
        `     ${src}  ${bytes(rawBytes)} raw, ${bytes(gzipBytes)} gzip  (${from.size === pages ? 'every page' : `${from.size} pages`})`,
      );
    }

    lines.splice(
      0,
      0,
      `scanned ${plural(pages, 'built marketing page')}: ${plural(referenced.size, 'referenced .js file')}, ` +
        `${plural(inline.length, 'inline script')}`,
    );
    lines.push(
      `${total <= JS_BUDGET_BYTES ? 'ok  ' : 'FAIL'} total ${bytes(total)} gzip (${bytes(raw)} raw) against a ${bytes(JS_BUDGET_BYTES)} budget — ${bytes(JS_BUDGET_BYTES - total)} ${total <= JS_BUDGET_BYTES ? 'headroom' : 'over'}`,
    );

    if (total > JS_BUDGET_BYTES) {
      problems.push(
        `client JS is ${bytes(total)} gzipped, over the ${bytes(JS_BUDGET_BYTES)} budget by ${bytes(total - JS_BUDGET_BYTES)}`,
      );
    }
    if (referenced.size === 0) {
      lines.push('     note: no script files referenced at all — the static figure is the whole demo');
    }

    if (problems.length > 0) {
      for (const problem of problems) lines.push(`FAIL ${problem}`);
      return { status: 'fail', lines };
    }
    return { status: 'pass', lines };
  },
};
