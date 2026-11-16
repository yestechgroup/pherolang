/**
 * Check 8 — compiler-backed highlighting, over the artifact and both built
 * trees.
 *
 * The contract is `docs/HIGHLIGHTING.md` (the "Site gate" section). The short
 * version:
 *
 *   1. `site/data/highlighting/spans.json` is a committed, format-versioned
 *      artifact containing spans and nothing else — no paths, timestamps or
 *      colours;
 *   2. the artifact covers every source the sites render, and holds no stale
 *      entry;
 *   3. every docs fence language is either a Phero surface or explicitly
 *      exempt;
 *   4. every Phero block in the built marketing and docs output carries
 *      `hl-*` classes (a raw `language-<surface>` block or a
 *      `data-phero-highlight="missing"` marker is a failure);
 *   5. no code block smuggles a `<script>` past the client-JS budget.
 *
 * The scanners are pure and exported: the unit tests in `scripts/test/` cover
 * them with synthetic HTML, so a failure here should name a page, a selector
 * or a fence line and never need the gate to be re-run to be understood.
 */

import { join } from 'node:path';

import {
  HIGHLIGHT_ARTIFACT_FORMAT_VERSION,
  keyFor,
} from '../../packages/highlighting/src/artifact.mjs';
import { EXEMPT_DOC_LANGS, isPheroSurface } from '../../packages/highlighting/src/taxonomy.mjs';
import { collectSources } from '../highlighting/sources.mjs';
import { APPS, DATA, REPO } from './paths.mjs';
import { isDir, isFile, plural, readText, walk } from './util.mjs';

const NAME = 'highlighting';

const PRE_BLOCK = /<pre\b([^>]*)>([\s\S]*?)<\/pre>/gi;
const CODE_OPEN = /<code\b([^>]*)>/i;
const CLASS_ATTRIBUTE = /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i;
const DATA_LANGUAGE_ATTRIBUTE = /\bdata-language\s*=\s*(?:"([^"]*)"|'([^']*)')/i;
const CLASS_ATTRIBUTES = /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/gi;
const MISSING_MARKER = /<[a-zA-Z][^>]*\sdata-phero-highlight\s*=\s*["']?missing["']?/i;

/** Read one quoted attribute from an HTML start tag's attribute text. */
function attribute(tag, pattern) {
  const match = pattern.exec(String(tag ?? ''));
  return match ? (match[1] ?? match[2] ?? null) : null;
}

/** The `language-*` token of a `class` attribute, lowercased. */
function languageFromClass(className) {
  if (!className) return null;
  for (const token of className.split(/\s+/)) {
    if (token.startsWith('language-')) return token.slice('language-'.length).toLowerCase();
  }
  return null;
}

/** True when any class attribute in `segment` carries an `hl-*` class. */
function hasHighlightClass(segment) {
  CLASS_ATTRIBUTES.lastIndex = 0;
  for (const match of String(segment ?? '').matchAll(CLASS_ATTRIBUTES)) {
    const value = match[1] ?? match[2] ?? '';
    if (/(?:^|\s)hl-[\w-]+/.test(value)) return true;
  }
  return false;
}

function hasMissingMarker(segment) {
  return MISSING_MARKER.test(String(segment ?? ''));
}

/**
 * Classify a docs fence language.
 *
 * `'phero'` means the highlighting pipeline owns the fence; `'exempt'` means it
 * is deliberately left to Expressive Code/Shiki or to plaintext; anything else
 * fails the gate so adding a language is an explicit decision.
 */
export function classifyFenceLanguage(language) {
  const normalized = String(language ?? '')
    .trim()
    .toLowerCase()
    .replace(/^\{\.?/, '')
    .replace(/\}$/, '')
    .replace(/^\./, '');
  if (isPheroSurface(normalized)) return 'phero';
  if (EXEMPT_DOC_LANGS.has(normalized)) return 'exempt';
  return 'unsupported';
}

/**
 * Scan built marketing HTML for Phero code blocks.
 *
 * Marketing renders `<pre>…<code class="language-<surface>">…</code></pre>`.
 * A Phero block must contain `hl-*` classes and must not carry
 * `data-phero-highlight="missing"`; a raw block is a finding.
 */
export function scanMarketingHtml(html) {
  const text = String(html ?? '');
  const blocks = [];
  const findings = [];

  for (const match of text.matchAll(PRE_BLOCK)) {
    const segment = match[0];
    const code = CODE_OPEN.exec(match[2]);
    if (!code) continue;

    const language = languageFromClass(attribute(code[1], CLASS_ATTRIBUTE));
    if (!language || !isPheroSurface(language)) continue;

    const highlighted = hasHighlightClass(segment);
    const missing = hasMissingMarker(segment);
    const raw = !highlighted && !missing;
    const index = blocks.length;
    blocks.push({ index, language, highlighted, missing, raw });

    if (missing) {
      findings.push(`block #${index} \`language-${language}\` is marked data-phero-highlight="missing"`);
    } else if (raw) {
      findings.push(`block #${index} \`language-${language}\` is raw (no hl-* classes)`);
    }
  }

  return { blocks, findings };
}

/**
 * Scan built docs HTML for Phero code blocks.
 *
 * Starlight/Expressive Code renders `<pre data-language="<surface>">…</pre>`.
 * Every Phero block must contain `hl-*` classes, and a page carrying the
 * `data-phero-highlight="missing"` marker fails as a whole.
 */
export function scanDocsHtml(html) {
  const text = String(html ?? '');
  const blocks = [];
  const findings = [];

  for (const match of text.matchAll(PRE_BLOCK)) {
    const segment = match[0];
    const language = attribute(match[1], DATA_LANGUAGE_ATTRIBUTE);
    if (!language || !isPheroSurface(language.toLowerCase())) continue;

    const highlighted = hasHighlightClass(segment);
    const missing = hasMissingMarker(segment);
    const raw = !highlighted && !missing;
    const index = blocks.length;
    blocks.push({ index, language: language.toLowerCase(), highlighted, missing, raw });

    if (raw) findings.push(`block #${index} \`${language}\` is raw (no hl-* classes)`);
  }

  if (hasMissingMarker(text)) {
    findings.push('page contains data-phero-highlight="missing"');
  }

  return { blocks, findings };
}

/**
 * Every `<pre>` block and the ones that contain a real `<script>` element.
 *
 * Code text is escaped by both renderers, so an unescaped `<script` inside a
 * `<pre>` is either a hand-written block or an injected island — the client-JS
 * budget check is strict about it.
 */
export function codeBlockScripts(html) {
  const hits = [];
  let blocks = 0;
  for (const match of String(html ?? '').matchAll(PRE_BLOCK)) {
    blocks += 1;
    if (/<script\b/i.test(match[0])) hits.push(blocks - 1);
  }
  return { blocks, hits };
}

const ARTIFACT_FORBIDDEN = [
  { label: 'a repository path (examples/ or docs/)', pattern: /(?:^|[^A-Za-z0-9_.-])(?:examples|docs)\// },
  { label: 'an absolute home path (/home/)', pattern: /\/home\// },
  { label: 'a hex colour', pattern: /#[0-9a-fA-F]{3,8}\b/ },
  {
    label: 'a timestamp',
    pattern:
      /\b(?:19|20)\d{2}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2})?)?/,
  },
  {
    label: 'a timestamp field',
    pattern: /(?:"|')?(?:timestamp|generatedAt|createdAt|updatedAt)(?:"|')?\s*:/i,
  },
];

/** A report line for one finding list, prefixed with the app-relative page. */
function pageFindings(prefix, findings) {
  return findings.map((finding) => `${prefix}: ${finding}`);
}

export const check = {
  name: NAME,
  title: 'compiler-backed highlighting covers every source, fence and built code block',

  run() {
    const lines = [];
    const problems = [];

    // --- 1. the committed artifact ------------------------------------
    if (!isFile(DATA.highlight)) {
      lines.push(`FAIL no highlight artifact at ${DATA.highlight}`);
      lines.push('     regenerate it with: node scripts/sync-highlight.mjs   (from site/)');
      return { status: 'fail', lines };
    }

    const text = readText(DATA.highlight);
    let artifact;
    try {
      artifact = JSON.parse(text);
    } catch (error) {
      lines.push(`FAIL invalid JSON in ${DATA.highlight}: ${error.message}`);
      return { status: 'fail', lines };
    }

    if (artifact?.formatVersion !== HIGHLIGHT_ARTIFACT_FORMAT_VERSION) {
      lines.push(
        `FAIL ${DATA.highlight} has formatVersion ${artifact?.formatVersion ?? 'absent'}; ` +
          `expected ${HIGHLIGHT_ARTIFACT_FORMAT_VERSION}`,
      );
      lines.push('     regenerate it with: node scripts/sync-highlight.mjs   (from site/)');
      return { status: 'fail', lines };
    }
    if (!artifact.entries || typeof artifact.entries !== 'object' || Array.isArray(artifact.entries)) {
      lines.push(`FAIL ${DATA.highlight} has no entries map`);
      return { status: 'fail', lines };
    }

    const entryCount = Object.keys(artifact.entries).length;
    const artifactFindings = [];
    for (const rule of ARTIFACT_FORBIDDEN) {
      const match = rule.pattern.exec(text);
      if (match) artifactFindings.push(`artifact contains ${rule.label}: ${JSON.stringify(match[0])}`);
    }
    if (artifactFindings.length === 0) {
      const generator = artifact.generator
        ? ` (${artifact.generator.name ?? 'unknown'} ${artifact.generator.version ?? 'unknown'})`
        : '';
      lines.push(`ok   artifact: ${plural(entryCount, 'entry', 'entries')}${generator}, path/colour/timestamp free`);
    } else {
      lines.push(`FAIL artifact: ${plural(artifactFindings.length, 'forbidden value')} in ${DATA.highlight}`);
      problems.push(...artifactFindings);
    }

    // --- 2. sources <-> artifact --------------------------------------
    let collected = null;
    try {
      collected = collectSources(REPO);
    } catch (error) {
      problems.push(`collecting the source corpus threw: ${String(error?.message ?? error).split('\n')[0]}`);
    }

    if (collected) {
      const { sources, problems: sourceProblems, docsFences } = collected;
      for (const problem of sourceProblems) problems.push(`source corpus: ${problem}`);

      const sourceKeys = new Map();
      for (const source of sources) {
        if (typeof source.source !== 'string') {
          problems.push(`${source.origin}: source text is not a string`);
          continue;
        }
        sourceKeys.set(keyFor(source.surface, source.source), source);
      }

      const missing = [...sourceKeys].filter(([key]) => !(key in artifact.entries));
      for (const [, source] of missing.slice(0, 20)) {
        problems.push(
          `no artifact entry for ${source.origin} (surface ${source.surface}) — run node scripts/sync-highlight.mjs`,
        );
      }
      if (missing.length > 20) problems.push(`… and ${missing.length - 20} more missing source entries`);

      const stale = Object.keys(artifact.entries).filter((key) => !sourceKeys.has(key));
      for (const key of stale.slice(0, 20)) {
        problems.push(`stale artifact entry ${key} maps to no collected source — run node scripts/sync-highlight.mjs`);
      }
      if (stale.length > 20) problems.push(`… and ${stale.length - 20} more stale entries`);

      if (missing.length === 0 && stale.length === 0) {
        lines.push(
          `ok   sources: ${plural(sourceKeys.size, 'collected source')} all have an entry and no artifact entry is stale`,
        );
      } else {
        lines.push(
          `FAIL sources: ${plural(missing.length, 'missing entry', 'missing entries')}, ` +
            `${plural(stale.length, 'stale entry', 'stale entries')}`,
        );
      }

      // --- 3. docs fence languages ------------------------------------
      const byKind = { phero: 0, exempt: 0, unsupported: 0 };
      const unsupported = [];
      for (const fence of docsFences) {
        const kind = classifyFenceLanguage(fence.lang);
        byKind[kind] += 1;
        if (kind === 'unsupported') {
          unsupported.push(
            `${fence.origin}: unsupported fence language ${JSON.stringify(fence.lang)} — ` +
              'use a Phero surface or add it to EXEMPT_DOC_LANGS',
          );
        }
      }
      if (unsupported.length === 0) {
        lines.push(
          `ok   docs fences: ${plural(docsFences.length, 'fence')} — ` +
            `${byKind.phero} Phero, ${byKind.exempt} exempt, 0 unsupported`,
        );
      } else {
        lines.push(`FAIL docs fences: ${plural(unsupported.length, 'unsupported language')}`);
        problems.push(...unsupported);
      }
    }

    // --- 4 + 5. built pages ------------------------------------------
    const scan = (app, scanHtml) => {
      if (!isDir(app.dist)) {
        problems.push(`${app.label}: no built tree at ${app.dist} — run the build check first`);
        lines.push(`FAIL ${app.label}: no built tree`);
        return;
      }

      const pages = walk(app.dist, (file) => file.endsWith('.html'));
      if (pages.length === 0) {
        problems.push(`${app.label}: no built pages under ${app.dist}`);
        lines.push(`FAIL ${app.label}: no built pages`);
        return;
      }

      const findings = [];
      let blocks = 0;
      let codeBlocks = 0;
      let scriptHits = 0;

      for (const rel of pages) {
        const html = readText(join(app.dist, rel));
        const scanned = scanHtml(html);
        blocks += scanned.blocks.length;
        findings.push(...pageFindings(`${app.key}:${rel}`, scanned.findings));

        const scripts = codeBlockScripts(html);
        codeBlocks += scripts.blocks;
        if (scripts.hits.length > 0) {
          scriptHits += scripts.hits.length;
          for (const index of scripts.hits) {
            findings.push(`${app.key}:${rel}: <script> inside code block #${index}`);
          }
        }
      }

      if (findings.length === 0) {
        lines.push(
          `ok   ${app.label}: ${plural(pages.length, 'page')}, ${plural(blocks, 'Phero block')} ` +
            `highlighted, no <script> in ${plural(codeBlocks, 'code block')}`,
        );
      } else {
        lines.push(
          `FAIL ${app.label}: ${plural(findings.length, 'finding')} across ${plural(pages.length, 'page')} ` +
            `(${blocks} Phero blocks, ${scriptHits} script hit${scriptHits === 1 ? '' : 's'})`,
        );
        problems.push(...findings);
      }
    };

    scan(APPS.web, scanMarketingHtml);
    scan(APPS.docs, scanDocsHtml);

    if (problems.length > 0) {
      lines.push(`FAIL ${plural(problems.length, 'highlighting problem')}:`);
      for (const problem of problems) lines.push(`     ${problem}`);
      return { status: 'fail', lines };
    }
    return { status: 'pass', lines };
  },
};
