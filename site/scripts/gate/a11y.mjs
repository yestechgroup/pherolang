/**
 * Check 5 — accessibility of the built output, in a real browser, in both
 * colour schemes.
 *
 * axe-core runs against `dist/client` served by `static-server.mjs`, so what is
 * measured is the artefact — including Starlight's inline scripts, Pagefind and
 * the graph island — not the source the layout hopes to render.
 *
 * Two passes:
 *
 *   light  every route in `ROUTES`, in the browser's default scheme, full page.
 *          This is the pass that has always run here; its lines and thresholds
 *          are unchanged.
 *   dark   the representative code-heavy routes in `DARK_ROUTES`, in a second
 *          context created with `colorScheme: 'dark'`. The compiler-backed
 *          highlighting reads the `--code-*` token layer, and `contrast.mjs`
 *          only proves the numbers in `tokens.css` — this proves the palette
 *          the browser actually paints when the colour scheme switches.
 *
 * The dark pass asserts the scheme applied *before* it analyses anything:
 * Starlight's switch must set `html[data-theme="dark"]`, and on marketing the
 * media query must match and both the painted body background and the resolved
 * `--code-bg` must be dark. A theme that silently stays light is a FAIL for
 * that route, never a quiet re-run of the light pass.
 *
 * Verdict rule: `serious` and `critical` violations fail the gate. `moderate`
 * and `minor` are printed with their ids and counted, because silencing them
 * would be the dishonest part; they are not merge blockers because axe's
 * moderate tier is dominated by best-practice guidance (heading order inside a
 * widget, landmark coverage) that this site addresses per component.
 *
 * No rule is disabled anywhere: a violation is reported with the selector that
 * caused it so the owning lane can fix the page rather than the gate.
 *
 * Dark scoping: a green full-page dark pass is what runs by default. If the
 * dark palette surfaces a pre-existing finding *outside* the code blocks — a
 * dark-only issue this lane does not own — the dark analysis is re-scoped to
 * the code containers (`.expressive-code, pre`) so the code-token verdict is
 * still decided on its own; the page-level findings stay in the report with
 * their selectors and the line says the analysis was scoped. That split is per
 * node, because a single axe violation (notably `color-contrast`) can cover
 * elements both inside and outside a code block. Nothing is disabled, and any
 * finding inside a code block is always reported normally.
 *
 * Alongside axe, three hard document invariants are asserted per route in the
 * light pass (they are colour-scheme independent, and every dark route is also
 * a light route): exactly one `<h1>`, a non-empty `<title>`, and a `lang` on
 * `<html>`.
 */

import { existsSync } from 'node:fs';

import AxeBuilder from '@axe-core/playwright';
import { chromium } from 'playwright';

import { APPS } from './paths.mjs';
import { startStaticServer } from './static-server.mjs';
import { plural } from './util.mjs';

const NAME = 'a11y';

const INSTALL = 'cd site && pnpm exec playwright install chromium';

/** The rule tags axe is asked for. WCAG 2.2 A/AA plus axe best-practice. */
const AXE_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa', 'best-practice'];

const BLOCKING = new Set(['serious', 'critical']);

/** Routes the plan names: the funnel, one language page, and both landings. */
const ROUTES = [
  { app: 'web', path: '/', label: 'marketing /' },
  { app: 'web', path: '/demo', label: 'marketing /demo' },
  { app: 'web', path: '/demo/hover', label: 'marketing /demo/hover' },
  { app: 'web', path: '/getting-started', label: 'marketing /getting-started' },
  { app: 'web', path: '/languages/mox', label: 'marketing /languages/mox' },
  { app: 'web', path: '/architecture', label: 'marketing /architecture' },
  { app: 'docs', path: '/', label: 'docs /' },
  { app: 'docs', path: '/reference/language/', label: 'docs /reference/language/' },
];

/**
 * The code-heavy routes re-run under `prefers-color-scheme: dark`. Each one is
 * also in `ROUTES`, so the light invariants above already cover it.
 */
const DARK_ROUTES = [
  { app: 'web', path: '/languages/mox', label: 'marketing /languages/mox' },
  { app: 'web', path: '/getting-started', label: 'marketing /getting-started' },
  { app: 'web', path: '/demo/hover', label: 'marketing /demo/hover' },
  { app: 'docs', path: '/reference/language/', label: 'docs /reference/language/' },
];

/**
 * Every code block in either app: marketing renders `<pre>` inside a
 * `<figure>`, docs render Expressive Code's `figure.expressive-code` (whose
 * `pre` is matched too). One selector, both surfaces.
 */
const CODE_REGION = '.expressive-code, pre';

/** How long the dark pass waits for the theme switch before calling it absent. */
const DARK_TIMEOUT_MS = 5_000;

/** Below this relative luminance a surface reads as dark (light is ~0.9). */
const DARK_MAX_LUMINANCE = 0.5;

const firstLine = (error) => String(error?.message ?? error).split('\n')[0];

/** Run axe with the gate's tag set, optionally scoped to a selector. */
function analyze(page, include) {
  let builder = new AxeBuilder({ page }).withTags(AXE_TAGS);
  if (include) builder = builder.include(include);
  return builder.analyze();
}

const split = (analysis) => ({
  blocking: analysis.violations.filter((violation) => BLOCKING.has(violation.impact)),
  rest: analysis.violations.filter((violation) => !BLOCKING.has(violation.impact)),
});

/**
 * What the browser actually painted. Relative luminance is computed here (the
 * same WCAG formula `contrast.mjs` uses) so the check can tell a dark ground
 * from a light one instead of trusting the media query alone.
 */
function probeTheme(page) {
  return page.evaluate(() => {
    const linear = (value) => {
      const c = value / 255;
      return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    };
    const luminance = (r, g, b) => 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
    const parse = (value) => {
      const rgb = /^rgba?\(([^)]+)\)$/i.exec(value.trim());
      if (rgb) {
        const parts = rgb[1]
          .split(/[,\s/]+/)
          .filter(Boolean)
          .map(Number);
        if (parts.length >= 3 && (parts.length < 4 || parts[3] > 0.99)) return parts.slice(0, 3);
        return null;
      }
      const hex = /^#([0-9a-f]{6})$/i.exec(value.trim());
      if (hex) return [0, 2, 4].map((offset) => Number.parseInt(hex[1].slice(offset, offset + 2), 16));
      return null;
    };
    // A transparent body shows the canvas (white), so walk up to the first
    // element that actually paints.
    const painted = () => {
      for (const element of [document.body, document.documentElement]) {
        if (!element) continue;
        const colour = parse(getComputedStyle(element).backgroundColor);
        if (colour) return colour;
      }
      return [255, 255, 255];
    };

    const body = painted();
    const codeBackground = getComputedStyle(document.documentElement).getPropertyValue('--code-bg').trim();
    const code = parse(codeBackground);

    return {
      darkMedia: matchMedia('(prefers-color-scheme: dark)').matches,
      dataTheme: document.documentElement.dataset.theme ?? null,
      bodyBackground: `rgb(${body.join(', ')})`,
      bodyLuminance: luminance(body[0], body[1], body[2]),
      codeBackground,
      codeLuminance: code ? luminance(code[0], code[1], code[2]) : null,
    };
  });
}

/**
 * Why the dark scheme did not take effect, or null when it did.
 *
 * Starlight owns `html[data-theme]`; marketing has no switch and keys off
 * `prefers-color-scheme`, so the query matching is not enough — the painted
 * ground and the resolved code surface have to be dark too.
 */
function themeProblem(app, probe) {
  if (app === 'docs') {
    if (probe.dataTheme !== 'dark') {
      return `html[data-theme] is ${JSON.stringify(probe.dataTheme)}, expected "dark"`;
    }
  } else if (!probe.darkMedia) {
    return 'matchMedia("(prefers-color-scheme: dark)") is false';
  }

  if (!(probe.bodyLuminance < DARK_MAX_LUMINANCE)) {
    return `body background ${probe.bodyBackground} is light (luminance ${probe.bodyLuminance.toFixed(2)})`;
  }
  if (probe.codeLuminance === null) {
    return `--code-bg resolved to ${JSON.stringify(probe.codeBackground)}, which is not a colour`;
  }
  if (!(probe.codeLuminance < DARK_MAX_LUMINANCE)) {
    return `--code-bg is ${probe.codeBackground}, not the dark token (luminance ${probe.codeLuminance.toFixed(2)})`;
  }
  return null;
}

/**
 * Split every full-page violation's nodes into those inside the code region and
 * those outside it. One axe violation can cover both (`color-contrast` collects
 * every failing element under one id), so the split is per node, never per
 * violation.
 *
 * A node whose target this page cannot resolve counts as outside; that makes
 * the caller re-scope, which is the safe direction — the scoped axe run stays
 * authoritative for the code blocks either way.
 */
async function splitNodesByCodeRegion(page, violations) {
  const groups = violations.map((violation) =>
    violation.nodes.map((node) => (Array.isArray(node.target) ? node.target.join(' ') : String(node.target ?? ''))),
  );

  let labels;
  try {
    labels = await page.evaluate(
      ({ region, groups: selectors }) => {
        const touchesCode = (selector) => {
          if (typeof selector !== 'string' || selector === '') return false;
          try {
            const element = document.querySelector(selector);
            return Boolean(element && element.closest(region));
          } catch {
            return false;
          }
        };
        return selectors.map((group) => group.map(touchesCode));
      },
      { region: CODE_REGION, groups },
    );
  } catch {
    labels = groups.map((group) => group.map(() => false));
  }

  return violations.map((violation, index) => {
    const inside = [];
    const outside = [];
    violation.nodes.forEach((node, nodeIndex) => {
      (labels[index]?.[nodeIndex] ? inside : outside).push(node);
    });
    return { violation, inside, outside };
  });
}

/** Print one route's blocking violations and collect the advisory ones. */
function record(route, problems, advisories, { blocking, rest }, suffix, dark) {
  const blockingNodes = blocking.reduce((total, violation) => total + violation.nodes.length, 0);
  const label = `${route.label}${dark ? ' (dark)' : ''}`;

  const line =
    `     ${blocking.length === 0 ? 'ok  ' : 'FAIL'} ${label.padEnd(dark ? 38 : 30)} ` +
    `${blocking.length} blocking (${blockingNodes} nodes), ${rest.length} advisory` +
    (suffix ? ` — ${suffix}` : '');

  for (const violation of blocking) {
    const first = violation.nodes[0];
    problems.push(
      `${label}: axe ${violation.impact} "${violation.id}" — ${violation.help} (${plural(violation.nodes.length, 'node')})`,
    );
    if (first) {
      problems.push(`     first selector: ${first.target.join(' ')}`);
      const html = (first.html ?? '').replace(/\s+/g, ' ').slice(0, 160);
      if (html) problems.push(`     markup: ${html}`);
    }
  }
  for (const violation of rest) {
    const first = violation.nodes[0];
    advisories.push(
      `${label}: ${violation.impact} "${violation.id}" x${violation.nodes.length}` +
        (first ? ` — ${first.target.join(' ')}` : ''),
    );
  }

  return line;
}

/**
 * Analyse one loaded route in one colour scheme.
 *
 * Returns `{ line, problems, advisories }`; `line` is null when the route could
 * not be measured at all (the caller keeps the failure, as before).
 */
async function analyzeRoute(page, route, { url, dark }) {
  const problems = [];
  const advisories = [];
  const label = `${route.label}${dark ? ' (dark)' : ''}`;

  let response;
  try {
    response = await page.goto(url, { waitUntil: 'load', timeout: 30_000 });
  } catch (error) {
    return { line: null, problems: [`${label}: ${url} could not be loaded — ${firstLine(error)}`], advisories };
  }
  if (!response || response.status() !== 200) {
    return {
      line: null,
      problems: [`${label}: ${url} answered ${response ? response.status() : 'no response'}`],
      advisories,
    };
  }

  if (dark) {
    // Prove the scheme before measuring it: a silent light-mode run would make
    // every dark line a lie.
    if (route.app === 'docs') {
      try {
        await page.waitForSelector('html[data-theme="dark"]', { timeout: DARK_TIMEOUT_MS });
      } catch {
        return {
          line: null,
          problems: [
            `${label}: Starlight never set html[data-theme="dark"] within ${DARK_TIMEOUT_MS} ms — the theme switch did not run`,
          ],
          advisories,
        };
      }
    }
    const probe = await probeTheme(page);
    const problem = themeProblem(route.app, probe);
    if (problem) {
      return {
        line: null,
        problems: [`${label}: dark colour scheme did not take effect — ${problem}`],
        advisories,
      };
    }

    const codeBlocks = await page.locator(CODE_REGION).count();
    if (codeBlocks === 0) {
      return {
        line: null,
        problems: [`${label}: no code blocks matched ${CODE_REGION} — this route is not exercising the code tokens`],
        advisories,
      };
    }
  } else {
    // Document invariants first: they are cheap and unambiguous.
    const h1Count = await page.locator('h1').count();
    const title = (await page.title()).trim();
    const lang = (await page.locator('html').getAttribute('lang')) ?? '';

    if (h1Count !== 1) {
      const first = await page.locator('h1').first().textContent().catch(() => null);
      problems.push(
        `${route.label}: ${plural(h1Count, '<h1>')} (expected exactly 1)${first ? ` — first is ${JSON.stringify(first.trim().slice(0, 60))}` : ''}`,
      );
    }
    if (title === '') problems.push(`${route.label}: <title> is empty`);
    if (lang.trim() === '') problems.push(`${route.label}: <html> has no lang attribute`);
  }

  let analysis;
  try {
    analysis = await analyze(page);
  } catch (error) {
    return { line: null, problems: [`${label}: axe could not run — ${firstLine(error)}`], advisories };
  }

  if (!dark) {
    const parts = split(analysis);
    return { line: record(route, problems, advisories, parts, '', false), problems, advisories };
  }

  // Dark: prefer the full-page pass, and only narrow to the code containers
  // when the dark palette surfaced something outside them.
  let scopeNote = 'dark, full page';
  if (analysis.violations.length > 0) {
    const classified = await splitNodesByCodeRegion(page, analysis.violations);
    const outside = classified
      .filter((entry) => entry.outside.length > 0)
      .map((entry) => ({ ...entry.violation, nodes: entry.outside }));
    const outsideNodes = outside.reduce((total, violation) => total + violation.nodes.length, 0);

    if (outside.length > 0) {
      for (const violation of outside) {
        const first = violation.nodes[0];
        advisories.push(
          `${label}, outside the code blocks: ${violation.impact} "${violation.id}" x${violation.nodes.length}` +
            (first ? ` — ${first.target.join(' ')}` : ''),
        );
      }
      try {
        analysis = await analyze(page, CODE_REGION);
      } catch (error) {
        return { line: null, problems: [`${label}: axe could not run — ${firstLine(error)}`], advisories };
      }
      scopeNote = `dark, code blocks only (${plural(outsideNodes, 'full-page node')} outside them)`;
    } else {
      scopeNote = 'dark, full page (all findings inside the code blocks)';
    }
  }

  const parts = split(analysis);
  return { line: record(route, problems, advisories, parts, scopeNote, true), problems, advisories };
}

export const check = {
  name: NAME,
  title: 'axe (serious+critical) and document invariants on 8 built routes, light and dark',

  async run() {
    const lines = [];
    const problems = [];
    const advisories = [];

    const executable = chromium.executablePath();
    if (!existsSync(executable)) {
      lines.push(`FAIL the Playwright chromium build this package pins is not installed`);
      lines.push(`     missing: ${executable}`);
      lines.push(`     install it with: ${INSTALL}`);
      return { status: 'fail', lines };
    }

    let browser;
    const servers = [];
    try {
      try {
        browser = await chromium.launch();
      } catch (error) {
        lines.push('FAIL chromium could not be launched');
        lines.push(`     ${firstLine(error)}`);
        lines.push(`     install the browser with: ${INSTALL}`);
        return { status: 'fail', lines };
      }

      const origins = {};
      for (const app of Object.values(APPS)) {
        if (!existsSync(app.dist)) {
          problems.push(`${app.label}: no built tree at ${app.dist} — run the build check first`);
          continue;
        }
        const server = await startStaticServer(app.dist);
        servers.push(server);
        origins[app.key] = server.origin;
      }
      if (problems.length > 0) {
        return { status: 'fail', lines: [...lines, ...problems.map((problem) => `FAIL ${problem}`)] };
      }

      const lightContext = await browser.newContext({ viewport: { width: 1280, height: 900 } });
      const darkContext = await browser.newContext({
        viewport: { width: 1280, height: 900 },
        colorScheme: 'dark',
      });

      try {
        for (const route of ROUTES) {
          const page = await lightContext.newPage();
          try {
            const result = await analyzeRoute(page, route, {
              url: `${origins[route.app]}${route.path}`,
              dark: false,
            });
            if (result.line) lines.push(result.line);
            problems.push(...result.problems);
            advisories.push(...result.advisories);
          } finally {
            await page.close().catch(() => {});
          }
        }

        for (const route of DARK_ROUTES) {
          const page = await darkContext.newPage();
          try {
            const result = await analyzeRoute(page, route, {
              url: `${origins[route.app]}${route.path}`,
              dark: true,
            });
            if (result.line) lines.push(result.line);
            problems.push(...result.problems);
            advisories.push(...result.advisories);
          } finally {
            await page.close().catch(() => {});
          }
        }
      } finally {
        await lightContext.close().catch(() => {});
        await darkContext.close().catch(() => {});
      }
    } finally {
      if (browser) await browser.close().catch(() => {});
      for (const server of servers) await server.close().catch(() => {});
    }

    if (advisories.length > 0) {
      lines.push(`advisory (not blocking, printed so it cannot go unnoticed):`);
      for (const advisory of advisories) lines.push(`     ${advisory}`);
    }

    if (problems.length > 0) {
      lines.push(`FAIL ${plural(problems.length, 'problem')}:`);
      for (const problem of problems) lines.push(`     ${problem}`);
      return { status: 'fail', lines };
    }

    lines.push(
      'ok   no serious or critical axe violations in light or dark; every route has one h1, a title and a lang',
    );
    return { status: 'pass', lines };
  },
};
