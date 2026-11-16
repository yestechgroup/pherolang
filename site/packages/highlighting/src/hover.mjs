/**
 * The SH7 static-hover index: authored symbol metadata, resolved against the
 * compiler's committed graph and the SH4 highlight artifact.
 *
 * This module is deliberately build-time only. It never runs in the browser
 * and it never creates a span: the spans come from `spans.json`, the symbol
 * identities come from `graph.json`, and the documentation links come from
 * `surfaces.json` + `docs-manifest.json`. The one authored layer is the small
 * `hover.json` index (which node to annotate, the signature to show, and one
 * sentence of prose).
 *
 * Both the Astro component and the site gate call `buildHoverSample` /
 * `validateHoverIndex`, so the page and the gate cannot disagree about what a
 * symbol means.
 */

import { existsSync, readFileSync } from 'node:fs';
import { extname, join } from 'node:path';

import { findRepoRoot, keyFor, lookupEntry } from './artifact.mjs';
import { buildByteMap } from './render.mjs';
import { HIGHLIGHT_CLASS_SET } from './taxonomy.mjs';

export const HOVER_INDEX_FORMAT_VERSION = 1;

/** Where every generated docs page lives. */
export const DOCS_ORIGIN = 'https://docs.pherolang.com';

/** Human labels for the graph node kinds the prototype can annotate. */
export const HOVER_KIND_LABELS = {
  class: 'class',
  interface: 'interface',
  enum: 'enum',
  datatype: 'datatype',
  vocabulary: 'vocabulary',
  entity: 'entity',
  value: 'value object',
  dto: 'DTO',
  service: 'service',
  repository: 'repository',
  operation: 'operation',
  serviceOperation: 'service operation',
  repositoryOperation: 'repository operation',
  capability: 'capability',
  event: 'event',
  channel: 'channel',
  decision: 'decision',
  case: 'case',
  task: 'task',
  view: 'view',
  component: 'component',
  action: 'action',
  command: 'command',
};

/** Nodes that satisfy the “class/type” half of the SH7 acceptance criterion. */
export const HOVER_TYPE_KINDS = new Set([
  'class',
  'interface',
  'enum',
  'datatype',
  'vocabulary',
  'entity',
  'value',
  'dto',
  'service',
  'repository',
]);

/** Nodes that satisfy the “capability/operation” half. */
export const HOVER_CALLABLE_KINDS = new Set([
  'operation',
  'serviceOperation',
  'repositoryOperation',
  'capability',
  'action',
  'command',
]);

const KEY_PATTERN = /^[a-z][a-z0-9-]*$/;

/** Collapse all whitespace runs so a signature can be compared line-safely. */
export function normalizeWhitespace(text) {
  return String(text ?? '').replace(/\s+/g, ' ').trim();
}

/** The display name of a graph node id's fully-qualified name. */
export function shortNodeName(name) {
  const parts = String(name ?? '').split('.');
  return parts[parts.length - 1] ?? '';
}

/** Read the authored index from the repository root. */
export function loadHoverIndex(root = findRepoRoot()) {
  if (!root) throw new Error('hover: cannot find the repository root');
  const path = join(root, 'site', 'packages', 'highlighting', 'hover.json');
  if (!existsSync(path)) throw new Error(`hover: missing ${path}`);
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    throw new Error(`hover: invalid JSON at ${path}: ${error.message}`);
  }
  if (!parsed || typeof parsed !== 'object') {
    throw new Error(`hover: ${path} must be an object`);
  }
  return parsed;
}

/** The text of one highlight span, using the artifact's UTF-8 byte offsets. */
export function spanText(source, span, byteMap = buildByteMap(source)) {
  const from = byteMap[span.start];
  const to = byteMap[span.end];
  if (from === undefined || to === undefined) return '';
  return source.slice(from, to);
}

/**
 * Resolve a graph node's file to the site's documentation page.
 *
 * The node's extension names a surface in `surfaces.json`; that surface names
 * a source document in `surfaces.json`; the docs manifest maps the source to
 * the built URL. A missing link is an error, never a guessed URL.
 */
function documentationFor(node, surfaces, docsManifest) {
  const surface = (surfaces?.surfaces ?? []).find((entry) => entry.extension === extname(node.file));
  if (!surface) return null;
  const page = (docsManifest?.generated ?? []).find((entry) => entry.source === surface.docs);
  if (!page) return null;
  return {
    url: new URL(page.url, DOCS_ORIGIN).href,
    label: `${surface.name} reference`,
    source: surface.docs,
    surface: surface.id,
  };
}

/** The label for a graph node kind; an unknown kind is reported, never hidden. */
export function kindLabel(kind) {
  return HOVER_KIND_LABELS[kind] ?? String(kind);
}

function checkIndexShape(index) {
  const problems = [];
  if (!index || typeof index !== 'object') {
    return ['the index is not an object'];
  }
  if (index.formatVersion !== HOVER_INDEX_FORMAT_VERSION) {
    problems.push(
      `formatVersion ${index.formatVersion ?? 'absent'} is not ${HOVER_INDEX_FORMAT_VERSION}`,
    );
  }
  if (!Array.isArray(index.examples) || index.examples.length === 0) {
    problems.push('examples must be a non-empty array');
  }
  return problems;
}

/**
 * Resolve one example against the compiler data.
 *
 * Returns `{ ok, errors, annotations, excerpt }`. An annotation carries the
 * display fields and the exact span byte range; `renderAnnotatedInlineHtml`
 * uses the range, the component uses the rest. Any problem makes the whole
 * example `ok: false` so the page can fall back to static code.
 */
export function buildHoverSample(example, { artifact, excerpt, graph, surfaces, docsManifest }) {
  const errors = [];
  const annotations = [];

  if (!example || typeof example !== 'object') {
    return { ok: false, errors: ['the example is not an object'], annotations, excerpt };
  }
  if (!KEY_PATTERN.test(String(example.id ?? ''))) {
    errors.push(`example id ${JSON.stringify(example.id)} must match ${KEY_PATTERN}`);
  }
  if (!example.surface) errors.push(`${example.id}: missing surface`);
  if (!example.excerpt) errors.push(`${example.id}: missing excerpt key`);
  if (!excerpt) {
    errors.push(`${example.id}: no excerpt resolved for ${JSON.stringify(example.excerpt)}`);
    return { ok: false, errors, annotations, excerpt };
  }
  if (excerpt.surface !== example.surface) {
    errors.push(
      `${example.id}: excerpt ${JSON.stringify(example.excerpt)} is ${excerpt.surface}, expected ${example.surface}`,
    );
  }
  if (!Array.isArray(example.symbols) || example.symbols.length === 0) {
    errors.push(`${example.id}: symbols must be a non-empty array`);
    return { ok: false, errors, annotations, excerpt };
  }

  const { entry, error } = lookupEntry(artifact, example.surface, excerpt.text);
  if (error) errors.push(`${example.id}: ${error}`);
  if (!entry) {
    errors.push(`${example.id}: no highlight-artifact entry for ${JSON.stringify(example.excerpt)}`);
    return { ok: false, errors, annotations, excerpt };
  }

  const nodes = new Map((graph?.nodes ?? []).map((node) => [node.id, node]));
  const seenKeys = new Set();
  const byteMap = buildByteMap(excerpt.text);
  const normalizedExcerpt = normalizeWhitespace(excerpt.text);

  for (const symbol of example.symbols) {
    const prefix = `${example.id}/${symbol?.key ?? '?'}`;
    if (!symbol || typeof symbol !== 'object') {
      errors.push(`${prefix}: symbol is not an object`);
      continue;
    }
    if (!KEY_PATTERN.test(String(symbol.key ?? ''))) {
      errors.push(`${prefix}: key ${JSON.stringify(symbol.key)} must match ${KEY_PATTERN}`);
      continue;
    }
    if (seenKeys.has(symbol.key)) {
      errors.push(`${prefix}: key is duplicated in this example`);
      continue;
    }
    seenKeys.add(symbol.key);

    for (const field of ['name', 'node', 'signature', 'summary']) {
      if (typeof symbol[field] !== 'string' || symbol[field].trim() === '') {
        errors.push(`${prefix}: ${field} must be a non-empty string`);
      }
    }
    if (!Array.isArray(symbol.classes) || symbol.classes.length === 0) {
      errors.push(`${prefix}: classes must be a non-empty array`);
      continue;
    }
    for (const className of symbol.classes) {
      if (!HIGHLIGHT_CLASS_SET.has(className)) {
        errors.push(`${prefix}: ${JSON.stringify(className)} is not a HighlightClass wire spelling`);
      }
    }

    const node = nodes.get(symbol.node);
    if (!node) {
      errors.push(`${prefix}: graph node ${JSON.stringify(symbol.node)} does not exist`);
      continue;
    }
    if (example.application && node.application !== example.application) {
      errors.push(
        `${prefix}: graph node is in application ${JSON.stringify(node.application)}, expected ${JSON.stringify(example.application)}`,
      );
    }
    if (shortNodeName(node.name) !== symbol.name) {
      errors.push(
        `${prefix}: graph node is named ${JSON.stringify(node.name)}, not ${JSON.stringify(symbol.name)}`,
      );
    }

    const normalizedSignature = normalizeWhitespace(symbol.signature);
    if (!normalizedExcerpt.includes(normalizedSignature)) {
      errors.push(`${prefix}: signature ${JSON.stringify(symbol.signature)} is not in the excerpt`);
    }

    const matches = entry.spans.filter((span) => {
      if (!symbol.classes.includes(span.class)) return false;
      if (symbol.declaration === true && !(span.modifiers ?? []).includes('declaration')) return false;
      return spanText(excerpt.text, span, byteMap) === symbol.name;
    });
    if (matches.length !== 1) {
      errors.push(
        `${prefix}: expected exactly one ${symbol.classes.join('/')} span for ${JSON.stringify(symbol.name)}, found ${matches.length}`,
      );
      continue;
    }

    const docs = documentationFor(node, surfaces, docsManifest);
    if (!docs) {
      errors.push(`${prefix}: no docs page for graph node file ${JSON.stringify(node.file)}`);
    }

    const match = matches[0];
    annotations.push({
      key: symbol.key,
      name: symbol.name,
      nodeId: node.id,
      nodeKind: node.kind,
      kindLabel: kindLabel(node.kind),
      signature: symbol.signature,
      summary: symbol.summary,
      docsUrl: docs?.url ?? null,
      docsLabel: docs?.label ?? null,
      docsSource: docs?.source ?? null,
      spanStart: match.start,
      spanEnd: match.end,
      targetId: `sym-${example.id}-${symbol.key}`,
      descriptionId: `desc-${example.id}-${symbol.key}`,
    });
  }

  const kinds = new Set(annotations.map((annotation) => annotation.nodeKind));
  if (![...kinds].some((kind) => HOVER_TYPE_KINDS.has(kind))) {
    errors.push(`${example.id}: no class/type node is annotated`);
  }
  if (![...kinds].some((kind) => HOVER_CALLABLE_KINDS.has(kind))) {
    errors.push(`${example.id}: no capability/operation node is annotated`);
  }

  return { ok: errors.length === 0, errors, annotations, excerpt };
}

/**
 * Validate the whole authored index against the committed compiler data.
 *
 * `excerpts` is an array or Map of `loadExcerpt` results. The return value is
 * `{ ok, problems, samples }`; `samples` is usable by the gate even when the
 * index has a problem, so the report can name every broken entry.
 */
export function validateHoverIndex(index, { excerpts, artifact, graph, surfaces, docsManifest }) {
  const problems = checkIndexShape(index);
  const samples = [];
  if (!Array.isArray(index?.examples)) {
    return { ok: false, problems, samples };
  }

  const byKey =
    excerpts instanceof Map
      ? excerpts
      : new Map((excerpts ?? []).map((excerpt) => [excerpt.key ?? excerpt.name, excerpt]));

  const ids = new Set();
  for (const example of index.examples) {
    if (example?.id && ids.has(example.id)) {
      problems.push(`example id ${JSON.stringify(example.id)} is duplicated`);
    }
    if (example?.id) ids.add(example.id);
    const excerpt = byKey.get(example?.excerpt);
    const result = buildHoverSample(example, {
      artifact,
      excerpt,
      graph,
      surfaces,
      docsManifest,
    });
    problems.push(...result.errors);
    samples.push({ example, excerpt: result.excerpt, annotations: result.annotations });
  }

  const allKinds = new Set(samples.flatMap((sample) => sample.annotations.map((a) => a.nodeKind)));
  if (![...allKinds].some((kind) => HOVER_TYPE_KINDS.has(kind))) {
    problems.push('the index has no class/type symbol');
  }
  if (![...allKinds].some((kind) => HOVER_CALLABLE_KINDS.has(kind))) {
    problems.push('the index has no capability/operation symbol');
  }

  return { ok: problems.length === 0, problems, samples };
}

/** A stable diagnostic for callers that only need the first reason. */
export function firstHoverProblem(result) {
  return result?.errors?.[0] ?? result?.problems?.[0] ?? 'the hover index could not be resolved';
}

/** Test-only: expose the content-addressed key for one excerpt. */
export function excerptKey(surface, source) {
  return keyFor(surface, source);
}
