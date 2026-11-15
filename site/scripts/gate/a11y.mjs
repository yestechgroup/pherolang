/**
 * Check 5 — accessibility of the built output, in a real browser.
 *
 * axe-core runs against `dist/client` served by `static-server.mjs`, so what is
 * measured is the artefact — including Starlight's inline scripts, Pagefind and
 * the graph island — not the source the layout hopes to render.
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
 * Alongside axe, three hard document invariants are asserted per route: exactly
 * one `<h1>`, a non-empty `<title>`, and a `lang` on `<html>`.
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
  { app: 'web', path: '/getting-started', label: 'marketing /getting-started' },
  { app: 'web', path: '/languages/mox', label: 'marketing /languages/mox' },
  { app: 'web', path: '/architecture', label: 'marketing /architecture' },
  { app: 'docs', path: '/', label: 'docs /' },
  { app: 'docs', path: '/reference/language/', label: 'docs /reference/language/' },
];

export const check = {
  name: NAME,
  title: 'axe (serious+critical) and document invariants on 7 built routes',

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
        lines.push(`     ${String(error.message ?? error).split('\n')[0]}`);
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

      const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });

      for (const route of ROUTES) {
        const page = await context.newPage();
        const url = `${origins[route.app]}${route.path}`;
        let response;
        try {
          response = await page.goto(url, { waitUntil: 'load', timeout: 30_000 });
        } catch (error) {
          problems.push(`${route.label}: ${url} could not be loaded — ${String(error.message ?? error).split('\n')[0]}`);
          await page.close();
          continue;
        }
        if (!response || response.status() !== 200) {
          problems.push(`${route.label}: ${url} answered ${response ? response.status() : 'no response'}`);
          await page.close();
          continue;
        }

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

        let analysis;
        try {
          analysis = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
        } catch (error) {
          problems.push(`${route.label}: axe could not run — ${String(error.message ?? error).split('\n')[0]}`);
          await page.close();
          continue;
        }

        const blocking = analysis.violations.filter((violation) => BLOCKING.has(violation.impact));
        const rest = analysis.violations.filter((violation) => !BLOCKING.has(violation.impact));

        const blockingNodes = blocking.reduce((total, violation) => total + violation.nodes.length, 0);
        lines.push(
          `     ${blocking.length === 0 ? 'ok  ' : 'FAIL'} ${route.label.padEnd(30)} ` +
            `${blocking.length} blocking (${blockingNodes} nodes), ${rest.length} advisory`,
        );

        for (const violation of blocking) {
          const first = violation.nodes[0];
          problems.push(
            `${route.label}: axe ${violation.impact} "${violation.id}" — ${violation.help} (${plural(violation.nodes.length, 'node')})`,
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
            `${route.label}: ${violation.impact} "${violation.id}" x${violation.nodes.length}` +
              (first ? ` — ${first.target.join(' ')}` : ''),
          );
        }

        await page.close();
      }

      await context.close();
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

    lines.push('ok   no serious or critical axe violations; every route has one h1, a title and a lang');
    return { status: 'pass', lines };
  },
};
