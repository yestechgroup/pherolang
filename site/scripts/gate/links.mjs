/**
 * Check 2 — link integrity over both built origins.
 *
 * The two apps are one website split across two hostnames, and nothing at
 * build time knows that. So the gate reads the built trees as the network
 * would: an `<a href>` is resolved against the tree that owns its origin, and
 * the only links that leave the machine are the ones the check is told to
 * count rather than fetch.
 *
 * Cross-origin rules, as deployed:
 *   * `pherolang.com` (marketing) links to `https://docs.pherolang.com/...`
 *     for every docs page — those resolve against `phero-docs/dist/client`;
 *   * `docs.pherolang.com` links back to `pherolang.com/...` — resolved
 *     against `phero-web/dist/client`;
 *   * anything else (`github.com`, `mailto:`, release downloads) is counted
 *     and shape-checked, never fetched: the gate has to pass on a machine with
 *     no network.
 *
 * The `/docs/*` redirect is the third contract: marketing serves no docs page,
 * so a `/docs/...` request is a 301 to the docs origin. Two independent things
 * have to hold — the rule must still be in `public/_redirects` (and therefore
 * in the built tree), and no marketing page may link to `/docs/...`, because
 * such a link would work only by round-tripping through a redirect the site
 * itself should not need.
 */

import { join } from 'node:path';

import { APPS, ORIGIN_APP, REDIRECTS_BUILT, REDIRECTS_SOURCE } from './paths.mjs';
import { anchors, ids } from './html-scan.mjs';
import { isFile, plural, readText, walk } from './util.mjs';

const NAME = 'links';

const SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;

/** URL path a built page is served at. */
function pageUrlPath(rel) {
  if (rel === 'index.html') return '/';
  if (rel.endsWith('/index.html')) return `/${rel.slice(0, -'index.html'.length)}`;
  return `/${rel}`;
}

/** Normalise `.`/`..` segments without touching the filesystem. */
function normalizeUrlPath(path) {
  const segments = [];
  for (const segment of path.split('/')) {
    if (segment === '' || segment === '.') continue;
    if (segment === '..') segments.pop();
    else segments.push(segment);
  }
  const trailing = path.endsWith('/') && segments.length > 0 ? '/' : '';
  return `/${segments.join('/')}${trailing}`;
}

/**
 * Resolve a URL path inside one built tree.
 *
 * Order matters: a directory with an `index.html` wins over a sibling `.html`
 * file of the same name, which is what a static host does. `null` means the
 * host would 404. Because the path is rebuilt from its segments, `..` can never
 * walk out of the tree — a traversal attempt resolves to a miss, not a read.
 */
function resolveInTree(dist, urlPath) {
  let decoded;
  try {
    decoded = decodeURIComponent(urlPath);
  } catch {
    return { file: null, reason: 'malformed percent-encoding' };
  }
  const clean = normalizeUrlPath(decoded);

  const candidates = clean.endsWith('/')
    ? [join(dist, clean, 'index.html')]
    : [join(dist, clean), join(dist, `${clean}/index.html`), join(dist, `${clean}.html`)];

  for (const candidate of candidates) {
    if (isFile(candidate)) return { file: candidate };
  }
  return { file: null, reason: `no file for ${clean}` };
}

function classify(href, pagePath) {
  const value = href.trim();
  if (value === '') return { kind: 'malformed', reason: 'empty href' };
  if (/^javascript:/i.test(value)) return { kind: 'malformed', reason: 'javascript: URL' };
  if (/[\s<>]/.test(value)) return { kind: 'malformed', reason: 'whitespace or raw angle brackets' };
  if (value.startsWith('#')) {
    return { kind: 'anchor', path: pagePath, hash: value.slice(1) };
  }
  if (SCHEME.test(value)) {
    let url;
    try {
      url = new URL(value);
    } catch {
      return { kind: 'malformed', reason: 'unparseable absolute URL' };
    }
    if (url.protocol === 'mailto:' || url.protocol === 'tel:') {
      const address = decodeURIComponent(url.pathname);
      if (address === '' || address.includes(' ')) {
        return { kind: 'malformed', reason: `${url.protocol} address is empty or contains a space` };
      }
      return { kind: 'external', url: value };
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return { kind: 'malformed', reason: `unsupported scheme ${url.protocol}` };
    }
    if (url.hostname === '' || url.hostname.includes('..')) {
      return { kind: 'malformed', reason: 'empty or malformed hostname' };
    }
    const targetApp = ORIGIN_APP[url.hostname];
    if (!targetApp) return { kind: 'external', url: value };
    return {
      kind: 'cross-origin',
      appKey: targetApp,
      path: url.pathname === '' ? '/' : url.pathname,
      hash: url.hash.startsWith('#') ? url.hash.slice(1) : '',
      url: value,
    };
  }
  const hashIndex = value.indexOf('#');
  const pathPart = hashIndex === -1 ? value : value.slice(0, hashIndex);
  const hash = hashIndex === -1 ? '' : value.slice(hashIndex + 1);
  const base = pathPart.startsWith('/')
    ? pathPart
    : `${pagePath.replace(/[^/]*$/, '')}${pathPart}`;
  return { kind: 'relative', path: base, hash };
}

/** `_redirects` rules as `{ from, to, status }`, comments and blanks dropped. */
function redirectRules(text) {
  return text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '' && !line.startsWith('#') && !line.startsWith('!'))
    .map((line) => {
      const [from, to, status] = line.split(/\s+/);
      return { from, to, status: status ?? '302' };
    });
}

export const check = {
  name: NAME,
  title: 'internal + cross-origin links resolve in the built trees; the /docs redirect holds',

  run() {
    const lines = [];
    const broken = [];
    const idCache = new Map();
    const counts = { pages: 0, internal: 0, cross: 0, external: 0, anchors: 0, uniques: new Set() };

    const idsOf = (file) => {
      if (!idCache.has(file)) idCache.set(file, ids(readText(file)));
      return idCache.get(file);
    };

    const anchorProblem = (file, hash) => {
      if (hash === '' || hash === undefined) return null;
      const set = idsOf(file);
      return set.has(hash) ? null : `#${hash} is not an id on ${file}`;
    };

    for (const app of Object.values(APPS)) {
      if (!isFile(join(app.dist, 'index.html'))) {
        broken.push({ from: app.dist, href: '(tree)', reason: 'built tree is missing — run the build check first' });
        continue;
      }

      const pages = walk(app.dist, (file) => file.endsWith('.html'));
      for (const rel of pages) {
        const pagePath = pageUrlPath(rel);
        const from = `${app.key}:${rel}`;
        const html = readText(join(app.dist, rel));
        counts.pages += 1;

        for (const href of anchors(html)) {
          counts.uniques.add(href);
          const target = classify(href, pagePath);

          if (target.kind === 'malformed') {
            broken.push({ from, href, reason: target.reason });
            continue;
          }
          if (target.kind === 'external') {
            counts.external += 1;
            continue;
          }

          const targetApp = target.kind === 'cross-origin' ? APPS[target.appKey] : app;
          if (target.kind === 'cross-origin') counts.cross += 1;
          else if (target.kind === 'anchor') counts.anchors += 1;
          else counts.internal += 1;

          const resolved = resolveInTree(targetApp.dist, target.path);
          if (!resolved.file) {
            broken.push({
              from,
              href,
              reason:
                target.kind === 'cross-origin'
                  ? `${resolved.reason} in the ${targetApp.label} tree`
                  : resolved.reason,
            });
            continue;
          }

          if (resolved.file.endsWith('.html')) {
            const problem = anchorProblem(resolved.file, target.hash);
            if (problem) broken.push({ from, href, reason: problem });
          }
        }
      }
    }

    // --- the /docs redirect contract -------------------------------------
    const redirectProblems = [];
    if (!isFile(REDIRECTS_SOURCE)) {
      redirectProblems.push(`${REDIRECTS_SOURCE} is missing`);
    } else {
      const source = readText(REDIRECTS_SOURCE);
      const rules = redirectRules(source);
      const docsRule = rules.find((rule) => rule.from === '/docs');
      const docsWildcard = rules.find((rule) => rule.from === '/docs/*');

      if (!docsRule) {
        redirectProblems.push('no `/docs` rule');
      } else if (docsRule.to !== 'https://docs.pherolang.com/' || docsRule.status !== '301') {
        redirectProblems.push(
          `\`/docs\` points at ${docsRule.to} (${docsRule.status}), expected https://docs.pherolang.com/ (301)`,
        );
      }
      if (!docsWildcard) {
        redirectProblems.push('no `/docs/*` rule');
      } else if (
        docsWildcard.to !== 'https://docs.pherolang.com/:splat' ||
        docsWildcard.status !== '301'
      ) {
        redirectProblems.push(
          `\`/docs/*\` points at ${docsWildcard.to} (${docsWildcard.status}), expected https://docs.pherolang.com/:splat (301)`,
        );
      }
      // The file documents that a rule only applies where it appears before the
      // broader one that would otherwise swallow it; assert the declared order.
      if (docsRule && docsWildcard) {
        const indexOf = (rule) => rules.indexOf(rule);
        if (indexOf(docsRule) > indexOf(docsWildcard)) {
          redirectProblems.push('`/docs` appears after `/docs/*` — the broad rule would match first');
        }
      }

      // A rule only applies if it survives into the deployed tree.
      if (!isFile(REDIRECTS_BUILT)) {
        redirectProblems.push(`the built tree has no _redirects (${REDIRECTS_BUILT})`);
      } else if (readText(REDIRECTS_BUILT) !== source) {
        redirectProblems.push('dist/client/_redirects differs from public/_redirects — rebuild');
      }
    }

    // No marketing page may link to `/docs/...`; the docs origin owns that path.
    for (const rel of walk(APPS.web.dist, (file) => file.endsWith('.html'))) {
      for (const href of anchors(readText(join(APPS.web.dist, rel)))) {
        if (/^\/docs(\/|$|\?|#)/.test(href.trim())) {
          broken.push({
            from: `web:${rel}`,
            href,
            reason: 'marketing links to /docs/... instead of https://docs.pherolang.com/...',
          });
        }
      }
    }

    // --- report -----------------------------------------------------------
    lines.push(
      `scanned ${plural(counts.pages, 'built page')}: ${plural(counts.internal, 'internal link')}, ` +
        `${plural(counts.cross, 'cross-origin link')}, ${plural(counts.external, 'external link')} ` +
        `(${counts.uniques.size} distinct href values, ${counts.anchors} same-page anchors)`,
    );

    if (redirectProblems.length > 0) {
      for (const problem of redirectProblems) lines.push(`FAIL redirect contract: ${problem}`);
    } else {
      lines.push(
        'ok   redirect contract: `/docs` and `/docs/*` both 301 to https://docs.pherolang.com, and the built copy matches',
      );
    }

    if (broken.length > 0) {
      lines.push(`FAIL ${plural(broken.length, 'broken link')}:`);
      for (const entry of broken) {
        lines.push(`     ${entry.from} -> ${entry.href}`);
        lines.push(`       ${entry.reason}`);
      }
    } else {
      lines.push('ok   every internal and cross-origin link resolves');
    }

    return {
      status: broken.length > 0 || redirectProblems.length > 0 ? 'fail' : 'pass',
      lines,
    };
  },
};

export { resolveInTree };
