/**
 * The one source list the highlight artifact is built from.
 *
 * Three buckets:
 *   1. every Phero Lang fence in `docs/**\/*.md`,
 *   2. every `examples/support/support.*` model,
 *   3. every exact excerpt in `site/packages/highlighting/excerpts.json`.
 *
 * Docs fences must equal the plaintext Expressive Code will hand the runtime
 * plugin (`split(/\r?\n/)`, trailing whitespace trimmed, leading/trailing blank
 * lines dropped). If they do not, fail with the file and line instead of
 * silently missing the lookup at build time.
 */

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

import { findRepoRoot } from '../../packages/highlighting/src/artifact.mjs';
import { allExcerptSpecs, loadExcerpt } from '../../packages/highlighting/src/excerpts.mjs';
import { isPheroSurface } from '../../packages/highlighting/src/taxonomy.mjs';
import { SURFACES } from '../surfaces.mjs';

const OPEN_FENCE = /^(\s{0,3})(`{3,}|~{3,})(.*)$/;
const CLOSE_FENCE = /^\s{0,3}(`{3,}|~{3,})\s*$/;

function posix(path) {
  return path.split(sep).join('/');
}

function walk(dir, predicate = () => true) {
  const out = [];
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(full, predicate));
    else if (entry.isFile() && predicate(full)) out.push(full);
  }
  return out;
}

/** Expressive Code's plaintext normalization, reproduced exactly. */
export function normalizeEcCode(code) {
  const lines = code.split(/\r?\n/).map((line) => line.trimEnd());
  while (lines.length && lines[0].length === 0) lines.shift();
  while (lines.length && lines[lines.length - 1].length === 0) lines.pop();
  return lines.join('\n');
}

function fenceLanguage(info) {
  const first = info.trim().split(/\s+/)[0] ?? '';
  // A Phero fence may carry a content-model suffix after the surface
  // (`mox,fragment`, `mox,invalid=E0300`). The suffix classifies the block for
  // the documentation gate; the highlighting pipeline keys on the surface.
  return first
    .replace(/^\{\.?/, '')
    .replace(/\}$/, '')
    .replace(/^\./, '')
    .split(',')[0]
    .toLowerCase();
}

/**
 * Scan every Markdown fence under `docs/`.
 *
 * Returns `{fences, problems}`. `fences` includes non-Phero languages so the
 * gate can prove each one is covered or explicitly exempt.
 */
export function collectDocsFences(root = findRepoRoot()) {
  const docsDir = join(root, 'docs');
  const fences = [];
  const problems = [];

  for (const absolute of walk(docsDir, (file) => file.endsWith('.md'))) {
    const text = readFileSync(absolute, 'utf8');
    const lines = text.split('\n');
    const file = posix(relative(root, absolute));

    let open = null;
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index];
      if (!open) {
        const match = OPEN_FENCE.exec(line);
        if (match) {
          open = {
            indent: match[1].length,
            marker: match[2][0],
            length: match[2].length,
            lang: fenceLanguage(match[3]),
            line: index + 1,
            content: [],
          };
        }
        continue;
      }

      const close = CLOSE_FENCE.exec(line);
      if (close && close[1][0] === open.marker && close[1].length >= open.length) {
        const raw = open.content.join('\n');
        const origin = `${file}:${open.line}`;
        if (isPheroSurface(open.lang)) {
          if (open.indent > 0) {
            problems.push(
              `${origin}: Phero fence \`${open.lang}\` is indented; the site pipeline requires an unindented fence`,
            );
          }
          const normalized = normalizeEcCode(raw);
          if (raw !== normalized) {
            problems.push(
              `${origin}: Phero fence \`${open.lang}\` differs from Expressive Code's rendered plaintext ` +
                '(remove trailing whitespace and leading/trailing blank lines)',
            );
          }
        }
        fences.push({ file, origin, lang: open.lang, raw });
        open = null;
        continue;
      }

      open.content.push(line);
    }

    if (open) problems.push(`${file}:${open.line}: unterminated code fence`);
  }

  return { fences, problems };
}

/** Every Phero `examples/support/support.*` source, keyed by its surface. */
export function collectExampleSources(root = findRepoRoot()) {
  const dir = join(root, 'examples', 'support');
  const byExtension = new Map(SURFACES.map((surface) => [surface.extension, surface.id]));
  const sources = [];

  if (existsSync(dir)) {
    for (const entry of readdirSync(dir).sort()) {
      if (!entry.startsWith('support.')) continue;
      const extension = entry.slice('support'.length);
      const surface = byExtension.get(extension);
      if (!surface) continue;
      const absolute = join(dir, entry);
      if (!statSync(absolute).isFile()) continue;
      sources.push({
        surface,
        source: readFileSync(absolute, 'utf8'),
        origin: `examples/support/${entry}`,
      });
    }
  }

  for (const surface of SURFACES) {
    if (!sources.some((entry) => entry.surface === surface.id)) {
      sources.push({
        surface: surface.id,
        source: null,
        origin: surface.example,
        missing: true,
      });
    }
  }
  return sources;
}

/** Every excerpt in the one manifest, as exact source bytes. */
export function collectExcerptSources(root = findRepoRoot()) {
  return allExcerptSpecs(root).map((spec) => {
    const excerpt = loadExcerpt(spec.key, root);
    return {
      surface: excerpt.surface,
      source: excerpt.text,
      origin: `excerpts.json#${spec.key}`,
    };
  });
}

/**
 * Every source the artifact must cover.
 *
 * `sources` may contain duplicate `(surface, source)` pairs; the artifact
 * builder deduplicates by key. `problems` is empty on a well-formed corpus.
 */
export function collectSources(root = findRepoRoot()) {
  if (!root) {
    return { sources: [], problems: ['cannot find the repository root'], docsFences: [] };
  }
  const docs = collectDocsFences(root);
  const problems = [...docs.problems];

  const docsSources = docs.fences
    .filter((fence) => isPheroSurface(fence.lang))
    .map((fence) => ({ surface: fence.lang, source: fence.raw, origin: fence.origin }));

  const examples = collectExampleSources(root);
  for (const example of examples) {
    if (example.missing) problems.push(`${example.origin}: missing Phero example source`);
  }

  let excerpts = [];
  try {
    excerpts = collectExcerptSources(root);
  } catch (error) {
    problems.push(String(error.message ?? error));
  }

  return {
    sources: [...examples.filter((entry) => !entry.missing), ...docsSources, ...excerpts],
    problems,
    docsFences: docs.fences,
  };
}
