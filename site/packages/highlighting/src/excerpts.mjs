/**
 * The one manifest of quoted source ranges.
 *
 * `catalog.ts` and the marketing pages read it to render an excerpt;
 * `scripts/sync-highlight.mjs` reads it to highlight the exact same bytes. A
 * range therefore cannot drift between the page and the artifact lookup.
 */

import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { findRepoRoot } from './artifact.mjs';

const manifestCache = new Map();

function manifestPath(root) {
  return join(root, 'site', 'packages', 'highlighting', 'excerpts.json');
}

/** Load the manifest (cached by repository root). */
export function loadExcerptManifest(root = findRepoRoot()) {
  if (!root) throw new Error('excerpts: cannot find the repository root');
  const path = manifestPath(root);
  if (manifestCache.has(path)) return manifestCache.get(path);

  if (!existsSync(path)) throw new Error(`excerpts: missing ${path}`);
  const manifest = JSON.parse(readFileSync(path, 'utf8'));
  if (!manifest.language || !manifest.pages) {
    throw new Error(`excerpts: ${path} must declare both "language" and "pages"`);
  }
  manifestCache.set(path, manifest);
  return manifest;
}

/** Every excerpt spec, with its key and group (`language` or `pages`). */
export function allExcerptSpecs(root = findRepoRoot()) {
  const manifest = loadExcerptManifest(root);
  const specs = [];
  for (const [key, spec] of Object.entries(manifest.language)) {
    specs.push({ key, group: 'language', ...spec });
  }
  for (const [key, spec] of Object.entries(manifest.pages)) {
    specs.push({ key, group: 'pages', ...spec });
  }
  return specs;
}

/** One excerpt spec by key (`mox` or `getting-started.domain`). */
export function excerptSpec(key, root = findRepoRoot()) {
  const manifest = loadExcerptManifest(root);
  const spec = manifest.language[key] ?? manifest.pages[key];
  if (!spec) throw new Error(`excerpts: no excerpt named '${key}'`);
  return { key, ...spec };
}

/**
 * Read and slice one excerpt.
 *
 * The text is `lines.slice(start - 1, end).join('\n')` — exactly what the
 * language page and both marketing walkthroughs render. The source path is
 * resolved from the repository root, never from the caller's cwd.
 */
export function loadExcerpt(key, root = findRepoRoot()) {
  const spec = excerptSpec(key, root);
  const absolute = join(root, spec.source);
  if (!existsSync(absolute)) {
    throw new Error(`excerpts: '${key}' points at missing source '${spec.source}'`);
  }

  const lines = readFileSync(absolute, 'utf8').split(/\r?\n/);
  const { start, end } = spec;
  if (!Number.isInteger(start) || !Number.isInteger(end) || start < 1 || end < start) {
    throw new Error(`excerpts: '${key}' has an invalid range ${start}–${end}`);
  }
  if (end > lines.length) {
    throw new Error(
      `excerpts: '${key}' cites ${spec.source} lines ${start}–${end}, but the file has ${lines.length} lines`,
    );
  }

  return { ...spec, text: lines.slice(start - 1, end).join('\n') };
}
