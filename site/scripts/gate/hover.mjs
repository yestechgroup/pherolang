/**
 * Check 9 — the SH7 static hover route.
 *
 * `/demo/hover` is the decision prototype for compiler-backed hover docs
 * without an editor. It is one page, two annotated examples and zero client
 * JavaScript; everything it shows is derived from committed compiler data at
 * build time. The contract has five parts, and each one is a distinct failure
 * mode:
 *
 *   1. pure      `hover.json` resolves against the committed highlight
 *                artifact, graph, surfaces table and docs manifest. Every
 *                symbol names a real graph node, its span exists exactly once,
 *                and its docs page resolves — no guessed URLs.
 *   2. static    the built page contains exactly the index's samples, every
 *                token anchor points at a real target and description id, and
 *                every card/permanent entry repeats the annotation's kind,
 *                signature, summary and docs link. There is no
 *                `data-hover-fallback` and no `<script>` element at all.
 *   3. budget    `hover.css` is gzipped at level 9 and must stay under 8 KB;
 *                the built page's own raw/gzip bytes are reported alongside.
 *   4. browser   in Chromium, focusing a symbol shows its card, marks its
 *                permanent entry, and Enter lands on the entry with the hash
 *                and focus the page promises. Hovering shows the card, the
 *                docs link resolves to `docs.pherolang.com`, the sr-only
 *                description is not hidden, axe finds no serious/critical
 *                violation, and the card still appears under
 *                `prefers-reduced-motion: reduce`.
 *   5. no motion `hover.css` introduces no `animation` or `transition`
 *                declaration; the card is an instant state change.
 *
 * The pure half runs even when the built page is absent, so a broken index is
 * reported without needing a build. A missing browser or built tree is a FAIL
 * with the command that fixes it, never a skip.
 */

import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

import AxeBuilder from '@axe-core/playwright';
import { chromium } from 'playwright';

import { loadArtifact } from '../../packages/highlighting/src/artifact.mjs';
import { allExcerptSpecs, loadExcerpt } from '../../packages/highlighting/src/excerpts.mjs';
import {
  loadHoverIndex,
  normalizeWhitespace,
  validateHoverIndex,
} from '../../packages/highlighting/src/hover.mjs';
import { APPS, DATA, REPO, SITE } from './paths.mjs';
import { startStaticServer } from './static-server.mjs';
import { bytes, isFile, plural, readJson, readText } from './util.mjs';

const NAME = 'hover';

const INSTALL = 'cd site && pnpm exec playwright install chromium';
const BUILD = 'node site/scripts/gate.mjs --only=build';

/** The route the page deploys under and the file that backs it. */
const ROUTE = '/demo/hover/';
const PAGE = join(APPS.web.dist, 'demo', 'hover', 'index.html');
const CSS = join(SITE, 'packages', 'highlighting', 'src', 'hover.css');
const GRAPH = join(DATA.dataDir, 'graph.json');

/** The SH7 acceptance budget for the hover stylesheet, gzipped. */
const CSS_BUDGET_BYTES = 8 * 1024;

/** The rule tags axe is asked for — the same set the a11y check uses. */
const AXE_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa', 'best-practice'];

const BLOCKING = new Set(['serious', 'critical']);

/** Any animation/transition declaration, including longhands. */
const MOTION = /\b(?:animation|transition)(?:-[\w-]+)?\s*:/i;

const firstLine = (error) => String(error?.message ?? error).split('\n')[0];

// --- HTML inspection -------------------------------------------------------
//
// The built page is already a byte-for-byte artefact; these helpers read the
// tags the component renders without pulling an HTML parser into the gate.

/** Every start tag in `html`, in document order, with its absolute offsets. */
function tagsOf(html) {
  const pattern = /<([a-zA-Z][\w:-]*)\b([^>]*)>/g;
  const tags = [];
  for (const match of String(html ?? '').matchAll(pattern)) {
    tags.push({
      name: match[1].toLowerCase(),
      attrs: match[2] ?? '',
      index: match.index ?? 0,
      end: (match.index ?? 0) + match[0].length,
    });
  }
  return tags;
}

/** One quoted attribute value from a start tag's attribute text. */
function attribute(tag, name) {
  if (!tag) return null;
  const pattern = new RegExp(`(?:^|\\s)${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`, 'i');
  const match = pattern.exec(tag.attrs);
  return match ? match[1] ?? match[2] : null;
}

/** The class tokens of a start tag. */
function classes(tag) {
  const value = attribute(tag, 'class');
  return value ? value.split(/\s+/).filter(Boolean) : [];
}

function hasClass(tag, name) {
  return classes(tag).includes(name);
}

function findTag(tags, name, predicate) {
  return tags.find((tag) => tag.name === name && predicate(tag)) ?? null;
}

/** The inside of `</name>` after `tag`, or null when the closing tag is absent. */
function bodyOf(html, tag, name) {
  const close = String(html ?? '').indexOf(`</${name}`, tag.end);
  return close === -1 ? null : String(html).slice(tag.end, close);
}

const ENTITY_PATTERN = /&(?:amp|lt|gt|quot|#39|#x27);/g;
const ENTITIES = {
  '&amp;': '&',
  '&lt;': '<',
  '&gt;': '>',
  '&quot;': '"',
  '&#39;': "'",
  '&#x27;': "'",
};

function decodeEntities(text) {
  return String(text ?? '').replace(ENTITY_PATTERN, (entity) => ENTITIES[entity]);
}

/** Visible text of an HTML fragment, whitespace-normalised. */
function textOf(fragment) {
  return normalizeWhitespace(decodeEntities(String(fragment ?? '').replace(/<[^>]*>/g, ' ')));
}

/** The text of the first `name.className` element, or null when it is absent. */
function elementText(html, tags, name, className) {
  const tag = findTag(tags, name, (candidate) => hasClass(candidate, className));
  if (!tag) return null;
  const body = bodyOf(html, tag, name);
  return body === null ? null : textOf(body);
}

/** Compare one required text field and report both sides on a mismatch. */
function expectText(problems, label, name, classNames, html, tags, expected) {
  const candidates = Array.isArray(classNames) ? classNames : [classNames];
  let actual = null;
  for (const className of candidates) {
    actual = elementText(html, tags, name, className);
    if (actual !== null) break;
  }
  if (actual === null) {
    problems.push(`${label}: no ${name}.${candidates.join(' / ')}`);
    return;
  }
  if (normalizeWhitespace(actual) !== normalizeWhitespace(expected ?? '')) {
    problems.push(
      `${label}: ${name}.${candidates[0]} is ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`,
    );
  }
}

/** Every rendered sample figure and its body. */
function sampleFigures(html) {
  const figures = [];
  for (const tag of tagsOf(html)) {
    if (tag.name !== 'figure') continue;
    const id = attribute(tag, 'data-hover-sample');
    if (id === null) continue;
    figures.push({ id, body: bodyOf(html, tag, 'figure') ?? '' });
  }
  return figures;
}

/**
 * Inspect the built page against the resolved samples.
 *
 * Returns `{ problems, figures, anchors, cards, entries }`; the counters are
 * what the pass line prints, and every problem names the sample, symbol,
 * selector and both values so the page can be fixed rather than the gate.
 */
export function inspectPage(html, samples) {
  const problems = [];
  const figures = sampleFigures(html);

  const found = figures.map((figure) => figure.id);
  const expected = samples.map((sample) => sample.example?.id ?? '');
  if (found.join('\n') !== expected.join('\n')) {
    problems.push(`samples ${JSON.stringify(found)} do not match the index ${JSON.stringify(expected)}`);
  }

  let anchors = 0;
  let cards = 0;
  let entries = 0;

  for (const sample of samples) {
    const id = sample.example?.id ?? '';
    const figure = figures.find((candidate) => candidate.id === id);
    if (!figure) {
      problems.push(`missing figure[data-hover-sample="${id}"]`);
      continue;
    }

    const tags = tagsOf(figure.body);
    const expectedCount = sample.annotations.length;
    const counts = {
      anchor: tags.filter((tag) => tag.name === 'a' && hasClass(tag, 'pherolang-hover__symbol')).length,
      card: tags.filter((tag) => tag.name === 'div' && hasClass(tag, 'pherolang-hover__card')).length,
      entry: tags.filter((tag) => tag.name === 'li' && hasClass(tag, 'pherolang-hover__entry')).length,
    };
    const labels = {
      anchor: ['token anchor', 'token anchors'],
      card: ['card', 'cards'],
      entry: ['permanent entry', 'permanent entries'],
    };
    for (const [kind, count] of Object.entries(counts)) {
      if (count !== expectedCount) {
        const [one, many] = labels[kind];
        problems.push(`${id}: ${plural(count, one, many)} in the page, expected ${expectedCount} (one per annotation)`);
      }
    }
    for (const annotation of sample.annotations) {
      const prefix = `${id}/${annotation.key}`;

      const anchor = findTag(
        tags,
        'a',
        (tag) => hasClass(tag, 'pherolang-hover__symbol') && attribute(tag, 'data-symbol') === annotation.key,
      );
      if (!anchor) {
        problems.push(`${prefix}: no token anchor a.pherolang-hover__symbol[data-symbol="${annotation.key}"]`);
      } else {
        anchors += 1;
        const href = attribute(anchor, 'href');
        if (href !== `#${annotation.targetId}`) {
          problems.push(`${prefix}: anchor href is ${JSON.stringify(href)}, expected "#${annotation.targetId}"`);
        }
        const describedBy = attribute(anchor, 'aria-describedby');
        if (describedBy !== annotation.descriptionId) {
          problems.push(
            `${prefix}: anchor aria-describedby is ${JSON.stringify(describedBy)}, expected "${annotation.descriptionId}"`,
          );
        }
      }

      const target = findTag(tags, 'li', (tag) => attribute(tag, 'id') === annotation.targetId);
      if (!target) {
        problems.push(`${prefix}: no permanent entry #${annotation.targetId}`);
      } else {
        entries += 1;
        if (!hasClass(target, 'pherolang-hover__entry')) {
          problems.push(`${prefix}: #${annotation.targetId} is not a .pherolang-hover__entry`);
        }
        const reference = attribute(target, 'data-reference');
        if (reference !== annotation.key) {
          problems.push(`${prefix}: entry data-reference is ${JSON.stringify(reference)}, expected "${annotation.key}"`);
        }
        const tabindex = attribute(target, 'tabindex');
        if (tabindex !== '-1') {
          problems.push(`${prefix}: entry tabindex is ${JSON.stringify(tabindex)}, expected "-1"`);
        }

        const body = bodyOf(figure.body, target, 'li') ?? '';
        const bodyTags = tagsOf(body);
        expectText(problems, prefix, 'h4', 'pherolang-hover__entry-name', body, bodyTags, annotation.name);
        expectText(
          problems,
          prefix,
          'p',
          ['pherolang-hover__entry-kind', 'pherolang-hover__kind'],
          body,
          bodyTags,
          annotation.kindLabel,
        );
        expectText(problems, prefix, 'p', 'pherolang-hover__entry-signature', body, bodyTags, annotation.signature);
        expectText(problems, prefix, 'p', 'pherolang-hover__entry-summary', body, bodyTags, annotation.summary);

        const link = findTag(
          bodyTags,
          'a',
          (tag) => hasClass(tag, 'pherolang-hover__entry-link'),
        );
        if (!link) {
          problems.push(`${prefix}: entry has no a.pherolang-hover__entry-link`);
        } else {
          const href = attribute(link, 'href');
          if (href !== annotation.docsUrl) {
            problems.push(`${prefix}: docs link is ${JSON.stringify(href)}, expected ${JSON.stringify(annotation.docsUrl)}`);
          }
          if (!String(href ?? '').startsWith('https://docs.pherolang.com/')) {
            problems.push(`${prefix}: docs link ${JSON.stringify(href)} does not start with https://docs.pherolang.com/`);
          }
        }
      }

      const description = findTag(tags, 'p', (tag) => attribute(tag, 'id') === annotation.descriptionId);
      if (!description) {
        problems.push(`${prefix}: no description p#${annotation.descriptionId}`);
      } else if (!hasClass(description, 'sr-only')) {
        problems.push(`${prefix}: description #${annotation.descriptionId} is not .sr-only`);
      }

      const card = findTag(
        tags,
        'div',
        (tag) => hasClass(tag, 'pherolang-hover__card') && attribute(tag, 'data-preview') === annotation.key,
      );
      if (!card) {
        problems.push(`${prefix}: no card div.pherolang-hover__card[data-preview="${annotation.key}"]`);
      } else {
        cards += 1;
        const hidden = attribute(card, 'aria-hidden');
        if (hidden !== 'true') {
          problems.push(`${prefix}: card aria-hidden is ${JSON.stringify(hidden)}, expected "true"`);
        }

        const body = bodyOf(figure.body, card, 'div') ?? '';
        const cardTags = tagsOf(body);
        expectText(problems, prefix, 'p', 'pherolang-hover__kind', body, cardTags, annotation.kindLabel);
        expectText(problems, prefix, 'p', 'pherolang-hover__signature', body, cardTags, annotation.signature);
        expectText(problems, prefix, 'p', 'pherolang-hover__summary', body, cardTags, annotation.summary);
        expectText(
          problems,
          prefix,
          'p',
          'pherolang-hover__reference-label',
          body,
          cardTags,
          `Reference: ${annotation.docsLabel}`,
        );
      }
    }
  }

  return { problems, figures: figures.length, anchors, cards, entries };
}

// --- browser ---------------------------------------------------------------

/** The computed paint that changes when an entry becomes the active one. */
const ENTRY_STYLE = (targetId) => {
  const element = document.getElementById(targetId);
  if (!element) return null;
  const style = getComputedStyle(element);
  return [style.outlineColor, style.outlineStyle, style.outlineWidth, style.borderColor, style.backgroundColor].join(' | ');
};

/** Whether the card for one symbol is painted, or null when it is missing. */
const CARD_VISIBLE = ({ sample, key }) => {
  const card = document.querySelector(
    `figure[data-hover-sample="${sample}"] .pherolang-hover__card[data-preview="${key}"]`,
  );
  return card ? getComputedStyle(card).display !== 'none' : null;
};

/** Whether the sr-only description is hidden by the two properties that matter. */
const DESCRIPTION_HIDDEN = (descriptionId) => {
  const element = document.getElementById(descriptionId);
  if (!element) return null;
  const style = getComputedStyle(element);
  return style.display === 'none' || style.visibility === 'hidden';
};

const tokenSelector = (sample, key) => `figure[data-hover-sample="${sample}"] a[data-symbol="${key}"]`;

/**
 * Exercise the built route in Chromium.
 *
 * Focus, hover, Enter and reduced motion are all DOM/CSS behaviours, so the
 * browser is the only honest place to decide them. Every assertion names the
 * sample and symbol that failed; nothing here throws for a page problem.
 */
async function exercise(browser, origin, samples) {
  const problems = [];
  const advisories = [];
  let annotations = 0;
  let hovered = 0;
  let blocking = 0;
  let axeRan = false;
  let darkBlocking = 0;
  let darkAxeRan = false;

  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  try {
    const page = await context.newPage();
    let response;
    try {
      response = await page.goto(`${origin}${ROUTE}`, { waitUntil: 'load', timeout: 30_000 });
    } catch (error) {
      return {
        problems: [`${ROUTE} could not be loaded — ${firstLine(error)}`],
        advisories,
        annotations,
        hovered,
        blocking,
      };
    }
    if (!response || response.status() !== 200) {
      return {
        problems: [`${ROUTE} answered ${response ? response.status() : 'no response'}`],
        advisories,
        annotations,
        hovered,
        blocking,
      };
    }

    for (const sample of samples) {
      const id = sample.example?.id ?? '';
      if (!id) {
        problems.push('a sample has no example id; the index cannot be exercised in the browser');
        continue;
      }

      for (const annotation of sample.annotations) {
        const prefix = `${id}/${annotation.key}`;

        // Clear any previous focus so the baseline is the inactive entry.
        await page.evaluate(() => {
          if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
        });

        const before = await page.evaluate(ENTRY_STYLE, annotation.targetId);
        if (before === null) {
          problems.push(`${prefix}: permanent entry #${annotation.targetId} does not exist in the browser`);
          continue;
        }

        try {
          await page.locator(tokenSelector(id, annotation.key)).focus();
        } catch (error) {
          problems.push(`${prefix}: token anchor could not be focused — ${firstLine(error)}`);
          continue;
        }

        const shown = await page.evaluate(CARD_VISIBLE, { sample: id, key: annotation.key });
        if (shown !== true) {
          problems.push(
            shown === null
              ? `${prefix}: no card[data-preview="${annotation.key}"] in the browser`
              : `${prefix}: card is still display:none while the symbol is focused`,
          );
        }

        const after = await page.evaluate(ENTRY_STYLE, annotation.targetId);
        if (after === null) {
          problems.push(`${prefix}: permanent entry #${annotation.targetId} disappeared while focused`);
        } else if (before === after) {
          problems.push(
            `${prefix}: permanent entry did not change outline/border/background while the symbol is focused`,
          );
        }

        await page.keyboard.press('Enter');
        try {
          await page.waitForFunction(
            ({ targetId }) =>
              window.location.hash === `#${targetId}` && document.activeElement?.id === targetId,
            { targetId: annotation.targetId },
            { timeout: 2_000 },
          );
        } catch {
          const state = await page.evaluate(() => ({
            hash: window.location.hash,
            active: document.activeElement?.id ?? null,
          }));
          problems.push(
            `${prefix}: Enter did not land on #${annotation.targetId} ` +
              `(hash ${JSON.stringify(state.hash)}, activeElement id ${JSON.stringify(state.active)})`,
          );
        }

        const docs = await page.evaluate((targetId) => {
          const entry = document.getElementById(targetId);
          const link = entry?.querySelector('a.pherolang-hover__entry-link');
          if (!link) return null;
          try {
            const url = new URL(link.href);
            return { href: link.href, protocol: url.protocol, hostname: url.hostname };
          } catch {
            return { href: link.href, protocol: null, hostname: null };
          }
        }, annotation.targetId);
        if (!docs) {
          problems.push(`${prefix}: entry has no docs link in the browser`);
        } else if (docs.protocol !== 'https:' || docs.hostname !== 'docs.pherolang.com') {
          problems.push(
            `${prefix}: docs link ${JSON.stringify(docs.href)} does not resolve to https://docs.pherolang.com/`,
          );
        }

        const hidden = await page.evaluate(DESCRIPTION_HIDDEN, annotation.descriptionId);
        if (hidden === null) {
          problems.push(`${prefix}: description #${annotation.descriptionId} is missing in the browser`);
        } else if (hidden) {
          problems.push(`${prefix}: description #${annotation.descriptionId} is display:none or visibility:hidden`);
        }

        annotations += 1;
      }

      const first = sample.annotations[0];
      if (first) {
        try {
          await page.locator(tokenSelector(id, first.key)).hover();
          const shown = await page.evaluate(CARD_VISIBLE, { sample: id, key: first.key });
          if (shown !== true) {
            problems.push(
              shown === null
                ? `${id}/${first.key}: no card to show on hover`
                : `${id}/${first.key}: card is still display:none on hover`,
            );
          } else {
            hovered += 1;
          }
        } catch (error) {
          problems.push(`${id}/${first.key}: token could not be hovered — ${firstLine(error)}`);
        }
      }
    }

    let analysis;
    try {
      analysis = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
    } catch (error) {
      problems.push(`axe could not run on ${ROUTE} — ${firstLine(error)}`);
      analysis = null;
    }
    if (analysis) {
      axeRan = true;
      const found = analysis.violations.filter((violation) => BLOCKING.has(violation.impact));
      blocking = found.reduce((total, violation) => total + violation.nodes.length, 0);
      for (const violation of found) {
        const firstNode = violation.nodes[0];
        problems.push(
          `axe ${violation.impact} "${violation.id}" on ${ROUTE} — ${violation.help} (${plural(violation.nodes.length, 'node')})`,
        );
        if (firstNode) problems.push(`     first selector: ${firstNode.target.join(' ')}`);
      }
      for (const violation of analysis.violations.filter((entry) => !BLOCKING.has(entry.impact))) {
        advisories.push(
          `axe ${violation.impact} "${violation.id}" x${violation.nodes.length} — not blocking, printed so it cannot go unnoticed`,
        );
      }
    }
  } finally {
    await context.close().catch(() => {});
  }

  const reducedContext = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    reducedMotion: 'reduce',
  });
  try {
    const page = await reducedContext.newPage();
    const response = await page.goto(`${origin}${ROUTE}`, { waitUntil: 'load', timeout: 30_000 });
    if (!response || response.status() !== 200) {
      problems.push(`reduced motion: ${ROUTE} answered ${response ? response.status() : 'no response'}`);
    } else {
      const first = samples[0]?.annotations[0];
      if (!first) {
        problems.push('reduced motion: the index has no annotation to focus');
      } else {
        const sampleId = samples[0].example?.id ?? '';
        try {
          await page.locator(tokenSelector(sampleId, first.key)).focus();
          const shown = await page.evaluate(CARD_VISIBLE, { sample: sampleId, key: first.key });
          if (shown !== true) {
            problems.push(
              shown === null
                ? 'reduced motion: no card[data-preview] in the browser'
                : 'reduced motion: the card is display:none with prefers-reduced-motion: reduce',
            );
          }
        } catch (error) {
          problems.push(`reduced motion: token could not be focused — ${firstLine(error)}`);
        }
      }
    }
  } finally {
    await reducedContext.close().catch(() => {});
  }

  const darkContext = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    colorScheme: 'dark',
  });
  try {
    const page = await darkContext.newPage();
    const response = await page.goto(`${origin}${ROUTE}`, { waitUntil: 'load', timeout: 30_000 });
    if (!response || response.status() !== 200) {
      problems.push(`dark axe: ${ROUTE} answered ${response ? response.status() : 'no response'}`);
    } else {
      const first = samples[0]?.annotations[0];
      if (!first) {
        problems.push('dark axe: the index has no annotation to focus');
      } else {
        const sampleId = samples[0].example?.id ?? '';
        try {
          await page.locator(tokenSelector(sampleId, first.key)).focus();
        } catch (error) {
          problems.push(`dark axe: token could not be focused — ${firstLine(error)}`);
        }
        try {
          const analysis = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
          darkAxeRan = true;
          const found = analysis.violations.filter((violation) => BLOCKING.has(violation.impact));
          darkBlocking = found.reduce((total, violation) => total + violation.nodes.length, 0);
          for (const violation of found) {
            const firstNode = violation.nodes[0];
            problems.push(
              `axe (dark, focused) ${violation.impact} "${violation.id}" on ${ROUTE} — ${violation.help} ` +
                `(${plural(violation.nodes.length, 'node')})`,
            );
            if (firstNode) problems.push(`     first selector: ${firstNode.target.join(' ')}`);
          }
          for (const violation of analysis.violations.filter((entry) => !BLOCKING.has(entry.impact))) {
            advisories.push(
              `axe (dark, focused) ${violation.impact} "${violation.id}" x${violation.nodes.length} — not blocking`,
            );
          }
        } catch (error) {
          problems.push(`dark axe could not run on ${ROUTE} — ${firstLine(error)}`);
        }
      }
    }
  } finally {
    await darkContext.close().catch(() => {});
  }

  return {
    problems,
    advisories,
    annotations,
    hovered,
    blocking,
    axeRan,
    darkBlocking,
    darkAxeRan,
  };
}

export const check = {
  name: NAME,
  title:
    'the /demo/hover prototype resolves its index and ships a zero-script page with an 8 KB CSS budget, keyboard access, axe and no motion',

  async run() {
    const lines = [];
    const problems = [];

    // --- 1. the pure index ---------------------------------------------
    let samples = null;
    try {
      const index = loadHoverIndex(REPO);
      const excerpts = new Map(allExcerptSpecs(REPO).map((spec) => [spec.key, loadExcerpt(spec.key, REPO)]));
      const { artifact, error } = loadArtifact(REPO);
      if (!artifact) {
        problems.push(`highlight artifact: ${error}`);
      } else {
        const graph = readJson(GRAPH);
        const surfaces = readJson(DATA.surfaces);
        const docsManifest = readJson(DATA.docsManifest);
        const result = validateHoverIndex(index, { excerpts, artifact, graph, surfaces, docsManifest });
        problems.push(...result.problems);
        samples = result.samples;
        if (result.ok) {
          const annotations = result.samples.reduce((total, sample) => total + sample.annotations.length, 0);
          lines.push(
            `ok   index: ${plural(result.samples.length, 'sample')}, ${plural(annotations, 'annotation')} — ` +
              'every symbol names a graph node, a unique span and a docs page',
          );
        }
      }
    } catch (error) {
      problems.push(`hover index: ${firstLine(error)}`);
    }

    // --- 2. the built page ---------------------------------------------
    let html = null;
    if (!isFile(PAGE)) {
      problems.push(`no built page at ${PAGE} — build it with: ${BUILD}`);
      lines.push('FAIL built page: missing');
    } else {
      html = readText(PAGE);
      const inspection = samples ? inspectPage(html, samples) : { problems: [], figures: 0, anchors: 0, cards: 0, entries: 0 };
      problems.push(...inspection.problems);

      const scripts = /<script\b/i.test(html);
      if (scripts) problems.push('built page contains a <script> element; the SH7 page must ship no client JavaScript');
      const fallback = /\bdata-hover-fallback\b/i.test(html);
      if (fallback) problems.push('built page contains data-hover-fallback; the index did not resolve at build time');

      if (!samples) {
        lines.push('     built page: not inspected against the index (the index did not resolve)');
      } else if (inspection.problems.length === 0 && !scripts && !fallback) {
        lines.push(
          `ok   built page: ${plural(inspection.figures, 'sample')}, ${plural(inspection.anchors, 'token anchor')}, ` +
            `${plural(inspection.cards, 'card')}, ${plural(inspection.entries, 'permanent entry', 'permanent entries')} — no <script>, no fallback marker`,
        );
      } else {
        lines.push(
          `FAIL built page: ${plural(inspection.problems.length, 'mismatch', 'mismatches')}` +
            `${scripts ? ', a <script>' : ''}${fallback ? ', data-hover-fallback' : ''}`,
        );
      }
    }

    // --- 3. the budget -------------------------------------------------
    if (!isFile(CSS)) {
      problems.push(`no hover stylesheet at ${CSS}`);
    } else {
      const css = readText(CSS);
      const raw = Buffer.byteLength(css, 'utf8');
      const gzipped = gzipSync(Buffer.from(css, 'utf8'), { level: 9 }).length;
      const within = gzipped < CSS_BUDGET_BYTES;
      const budgetLine =
        `hover.css: ${bytes(raw)} raw (${raw} B), ${bytes(gzipped)} gzip (${gzipped} B); ` +
        `budget ${bytes(CSS_BUDGET_BYTES)} (${CSS_BUDGET_BYTES} B)`;
      if (within) {
        lines.push(`ok   ${budgetLine}`);
      } else {
        problems.push(`hover.css gzips to ${gzipped} bytes; the budget is ${CSS_BUDGET_BYTES} bytes`);
        lines.push(`FAIL ${budgetLine}`);
      }

      // --- 5. no motion ------------------------------------------------
      const motion = MOTION.exec(css);
      if (motion) {
        const line = css.slice(0, motion.index).split('\n').length;
        problems.push(`hover.css:${line}: declares ${JSON.stringify(motion[0])}; the hover surface is no-motion`);
        lines.push(`FAIL no motion: hover.css:${line} declares ${JSON.stringify(motion[0])}`);
      } else {
        lines.push('ok   no motion: hover.css declares no animation or transition');
      }
    }

    if (html !== null) {
      const raw = Buffer.byteLength(html, 'utf8');
      const gzipped = gzipSync(Buffer.from(html, 'utf8'), { level: 9 }).length;
      lines.push(`     built page: ${bytes(raw)} raw (${raw} B), ${bytes(gzipped)} gzip (${gzipped} B) — report only`);
    }

    // --- 4. the browser -------------------------------------------------
    if (html !== null) {
      const executable = chromium.executablePath();
      if (!existsSync(executable)) {
        problems.push(
          `the Playwright chromium build this package pins is not installed (${executable}) — install it with: ${INSTALL}`,
        );
      } else if (!isFile(PAGE) || !samples) {
        // The static half already failed; the browser has nothing to assert.
      } else {
        let browser;
        let server;
        try {
          try {
            browser = await chromium.launch();
          } catch (error) {
            problems.push(`chromium could not be launched — ${firstLine(error)}; install it with: ${INSTALL}`);
          }
          if (browser) {
            server = await startStaticServer(APPS.web.dist);
            const result = await exercise(browser, server.origin, samples);
            problems.push(...result.problems);
            if (result.problems.length === 0) {
              lines.push(
                `ok   browser: ${plural(result.annotations, 'annotation')} — focus shows the card and marks the entry, ` +
                  `Enter lands on the target, hover confirmed on ${plural(result.hovered, 'sample')}, docs links and sr-only text hold`,
              );
            } else {
              lines.push(`FAIL browser: ${plural(result.problems.length, 'finding')} on ${ROUTE}`);
            }
            if (result.axeRan) {
              if (result.blocking === 0) {
                lines.push(`ok   axe: no serious or critical violations on ${ROUTE}`);
              } else {
                lines.push(`FAIL axe: ${plural(result.blocking, 'serious/critical node')} on ${ROUTE}`);
              }
            } else {
              lines.push(`     axe: not run (${ROUTE} did not load)`);
            }
            if (result.darkAxeRan) {
              if (result.darkBlocking === 0) {
                lines.push(`ok   axe (dark, focused): no serious or critical violations on ${ROUTE}`);
              } else {
                lines.push(
                  `FAIL axe (dark, focused): ${plural(result.darkBlocking, 'serious/critical node')} on ${ROUTE}`,
                );
              }
            } else {
              lines.push('     axe (dark, focused): not run');
            }
            for (const advisory of result.advisories) lines.push(`     advisory: ${advisory}`);
          }
        } catch (error) {
          problems.push(`browser check threw: ${firstLine(error)}`);
        } finally {
          if (browser) await browser.close().catch(() => {});
          if (server) await server.close().catch(() => {});
        }
      }
    }

    if (problems.length > 0) {
      lines.push(`FAIL ${plural(problems.length, 'hover problem')}:`);
      for (const problem of problems) lines.push(`     ${problem}`);
      return { status: 'fail', lines };
    }

    return { status: 'pass', lines };
  },
};
