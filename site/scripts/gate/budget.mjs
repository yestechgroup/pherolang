/**
 * Check 6 — the client-JS budget and the zero-JS page rule.
 *
 * The marketing site is static-first: every page renders complete HTML and the
 * only shipped script is the graph island on `/demo`. This check does two
 * things:
 *
 *   1. **Zero-JS pages.** Every other built marketing page must reference no
 *      executable script and contain no inline executable script. A second
 *      island, an analytics bundle or an Astro prefetch runtime is a finding,
 *      not a silent budget increase. (`<script type="application/json">` is
 *      data, not code, and is ignored.)
 *   2. **The budget.** The gzipped bytes of every distinct `.js` file the pages
 *      reference stay under `JS_BUDGET_BYTES`, so the one documented island
 *      cannot grow unnoticed.
 *
 * Gzip is measured at level 9 over the exact bytes the origin serves. The SH7
 * hover surface ships no JavaScript at all; it is covered by the same rule and
 * by its own gate.
 */

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

import { APPS, JS_BUDGET_BYTES } from './paths.mjs';
import { scripts } from './html-scan.mjs';
import { bytes, isFile, plural, readText, walk } from './util.mjs';

const NAME = 'budget';

const gzipped = (buffer) => gzipSync(buffer, { level: 9 }).length;

/**
 * The one documented client-JS island: the graph explorer on `/demo`
 * (SH7 and the hover page are static and ship no script). The key is the
 * page path relative to `dist/client`, POSIX separators.
 */
export const JS_ISLAND_PAGES = new Set(['demo/index.html']);

/** True when a `<script>` element runs code (as opposed to JSON data). */
export function isExecutableScript(attributes) {
  if (typeof attributes.src === 'string' && attributes.src !== '') return true;
  const type = String(attributes.type ?? '').trim().toLowerCase();
  return type === '' || type === 'module' || type.includes('javascript') || type.includes('ecmascript');
}

/**
 * Split one page's `<script>` elements into external sources and inline
 * executable scripts. Pure, so the unit tests can pin the classification.
 */
export function classifyPageScripts(html) {
  const external = [];
  const inline = [];
  for (const attributes of scripts(html)) {
    if (!isExecutableScript(attributes)) continue;
    if (typeof attributes.src === 'string' && attributes.src !== '') external.push(attributes.src);
    else inline.push(attributes);
  }
  return { external, inline };
}

export const check = {
  name: NAME,
  title: `static marketing pages ship no client JS except the /demo island (${JS_BUDGET_BYTES / 1024} KB gzip)`,

  run() {
    const lines = [];
    const problems = [];
    const referenced = new Map();
    const inline = [];
    const pagesWithScripts = [];
    let pages = 0;

    for (const rel of walk(APPS.web.dist, (file) => file.endsWith('.html'))) {
      const html = readText(join(APPS.web.dist, rel));
      pages += 1;
      const classified = classifyPageScripts(html);
      for (const src of classified.external) {
        if (!referenced.has(src)) referenced.set(src, new Set());
        referenced.get(src).add(rel);
      }
      if (classified.external.length > 0 || classified.inline.length > 0) {
        pagesWithScripts.push(rel);
      }
      if (classified.inline.length > 0) {
        // Inline executable scripts are reported and, outside the island,
        // failures: a static page has no reason to run one.
        inline.push({ page: rel, count: classified.inline.length });
      }
      if (!JS_ISLAND_PAGES.has(rel)) {
        for (const src of classified.external) {
          problems.push(
            `${rel} loads ${src}, but only the documented graph island (${[...JS_ISLAND_PAGES].join(', ')}) may ship client JS`,
          );
        }
        if (classified.inline.length > 0) {
          problems.push(
            `${rel} contains ${plural(classified.inline.length, 'inline executable script')}; static pages are zero-JS`,
          );
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
        `${plural(pagesWithScripts.length, 'page with a script', 'pages with a script')}, ` +
        `${plural(inline.length, 'page with an inline script', 'pages with an inline script')}`,
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
