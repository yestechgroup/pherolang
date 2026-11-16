import assert from 'node:assert/strict';
import { join } from 'node:path';
import test from 'node:test';

import { keyFor, loadArtifact, sha256Hex } from '../../packages/highlighting/src/artifact.mjs';
import { allExcerptSpecs, loadExcerpt } from '../../packages/highlighting/src/excerpts.mjs';
import {
  HOVER_CALLABLE_KINDS,
  HOVER_TYPE_KINDS,
  buildHoverSample,
  loadHoverIndex,
  normalizeWhitespace,
  shortNodeName,
  validateHoverIndex,
} from '../../packages/highlighting/src/hover.mjs';
import { renderAnnotatedInlineHtml, renderInlineHtml } from '../../packages/highlighting/src/render.mjs';
import { REPO } from '../gate/paths.mjs';
import { readJson } from '../gate/util.mjs';

/* ------------------------------------------------------------------ */
/* renderAnnotatedInlineHtml                                           */
/* ------------------------------------------------------------------ */

const SAMPLE_SOURCE = 'package demo\n\nclass Book {}\n';
// 'package' is bytes 0–7, 'Book' is bytes 20–24.
const SAMPLE_SPANS = [
  { start: 0, end: 7, class: 'keyword', modifiers: [] },
  { start: 20, end: 24, class: 'type', modifiers: ['declaration'] },
];

test('renderAnnotatedInlineHtml equals renderInlineHtml when nothing is annotated', () => {
  assert.equal(
    renderAnnotatedInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS, []),
    renderInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS),
  );
  assert.equal(
    renderAnnotatedInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS),
    renderInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS),
  );
});

test('renderAnnotatedInlineHtml wraps exactly the annotated span, keeping every class', () => {
  const annotations = [
    {
      key: 'book',
      spanStart: 20,
      targetId: 'sym-demo-book',
      descriptionId: 'desc-demo-book',
    },
  ];
  assert.equal(
    renderAnnotatedInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS, annotations),
    '<span class="hl-keyword">package</span> demo\n\nclass ' +
      '<a class="pherolang-hover__symbol" href="#sym-demo-book" ' +
      'data-symbol="book" aria-describedby="desc-demo-book">' +
      '<span class="hl-type hl-declaration">Book</span></a> {}\n',
  );
});

test('an annotation whose spanStart does not exist is ignored, never guessed', () => {
  const annotations = [
    { key: 'stale', spanStart: 9999, targetId: 'sym-stale', descriptionId: 'desc-stale' },
  ];
  assert.equal(
    renderAnnotatedInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS, annotations),
    renderInlineHtml(SAMPLE_SOURCE, SAMPLE_SPANS),
  );
});

test('renderAnnotatedInlineHtml escapes both the text and every attribute', () => {
  const source = `<b>&"'`;
  const spans = [{ start: 0, end: 3, class: 'string', modifiers: [] }];
  const annotations = [
    { key: 'k&"', spanStart: 0, targetId: 't<&"', descriptionId: "d'<" },
  ];
  assert.equal(
    renderAnnotatedInlineHtml(source, spans, annotations),
    '<a class="pherolang-hover__symbol" href="#t&lt;&amp;&quot;" ' +
      'data-symbol="k&amp;&quot;" aria-describedby="d&#39;&lt;">' +
      '<span class="hl-string">&lt;b&gt;</span></a>' +
      '&amp;&quot;&#39;',
  );
});

test('non-ASCII text keeps its UTF-8 byte offsets when annotated', () => {
  const source = 'é😀x';
  // 'é' is 2 bytes, '😀' is 4, 'x' is 1: the span starts at byte 6.
  const spans = [{ start: 6, end: 7, class: 'keyword', modifiers: [] }];
  const annotations = [
    { key: 'x', spanStart: 6, targetId: 'sym-x', descriptionId: 'desc-x' },
  ];
  assert.equal(
    renderAnnotatedInlineHtml(source, spans, annotations),
    'é😀<a class="pherolang-hover__symbol" href="#sym-x" ' +
      'data-symbol="x" aria-describedby="desc-x">' +
      '<span class="hl-keyword">x</span></a>',
  );
});

/* ------------------------------------------------------------------ */
/* normalizeWhitespace / shortNodeName                                 */
/* ------------------------------------------------------------------ */

test('normalizeWhitespace collapses every whitespace run and trims', () => {
  assert.equal(normalizeWhitespace('  class\n\tBook   {} '), 'class Book {}');
  assert.equal(normalizeWhitespace(''), '');
  assert.equal(normalizeWhitespace(null), '');
  assert.equal(normalizeWhitespace(42), '42');
});

test('shortNodeName returns the last dotted segment', () => {
  assert.equal(shortNodeName('nz.example.support.Ticket'), 'Ticket');
  assert.equal(shortNodeName('Ticket'), 'Ticket');
  assert.equal(shortNodeName(''), '');
  assert.equal(shortNodeName(null), '');
});

/* ------------------------------------------------------------------ */
/* buildHoverSample — a synthetic clean example                        */
/* ------------------------------------------------------------------ */

// The acceptance criterion needs both halves, so the sample carries a class
// and one operation; a single symbol can never satisfy it.
const SYNTHETIC_SOURCE = 'package demo\n\nclass Book {\n  op boolean borrow(int days) { true }\n}\n';
const SYNTHETIC_BOOK_END = SYNTHETIC_SOURCE.indexOf('Book') + 'Book'.length;
const SYNTHETIC_BORROW_START = SYNTHETIC_SOURCE.indexOf('borrow');
const SYNTHETIC_BORROW_END = SYNTHETIC_BORROW_START + 'borrow'.length;

function syntheticArtifact(surface = 'mox', source = SYNTHETIC_SOURCE, spans = null) {
  const key = keyFor(surface, source);
  return {
    formatVersion: 1,
    entries: {
      [key]: {
        surface,
        sourceHash: `sha256:${sha256Hex(source)}`,
        degraded: false,
        spans:
          spans ??
          [
            { start: SYNTHETIC_SOURCE.indexOf('Book'), end: SYNTHETIC_BOOK_END, class: 'type', modifiers: ['declaration'] },
            { start: SYNTHETIC_BORROW_START, end: SYNTHETIC_BORROW_END, class: 'function', modifiers: ['declaration'] },
          ],
      },
    },
  };
}

function syntheticGraph() {
  return {
    nodes: [
      {
        id: 'class:demo.mox#demo.Book',
        kind: 'class',
        name: 'demo.Book',
        file: 'support/support.mox',
        application: 'App',
      },
      {
        id: 'operation:demo.mox#demo.Book.borrow',
        kind: 'operation',
        name: 'demo.Book.borrow',
        file: 'support/support.mox',
        application: 'App',
      },
    ],
  };
}

function syntheticSurfaces() {
  return {
    surfaces: [{ id: 'mox', extension: '.mox', name: 'Domain model', docs: 'docs/LANGUAGE.md' }],
  };
}

function syntheticDocsManifest() {
  return {
    generated: [{ source: 'docs/LANGUAGE.md', url: '/reference/language/' }],
  };
}

function syntheticExcerpt(surface = 'mox') {
  return { key: 'demo', surface, text: SYNTHETIC_SOURCE };
}

function syntheticExample() {
  return {
    id: 'demo-book',
    surface: 'mox',
    excerpt: 'demo',
    application: 'App',
    symbols: [
      {
        key: 'book',
        name: 'Book',
        node: 'class:demo.mox#demo.Book',
        classes: ['type'],
        declaration: true,
        signature: 'class Book',
        summary: 'A book in the demo catalogue.',
      },
      {
        key: 'borrow',
        name: 'borrow',
        node: 'operation:demo.mox#demo.Book.borrow',
        classes: ['function'],
        declaration: true,
        signature: 'op boolean borrow(int days)',
        summary: 'Borrows the book for a number of days.',
      },
    ],
  };
}

function syntheticInputs(overrides = {}) {
  return {
    artifact: syntheticArtifact(),
    excerpt: syntheticExcerpt(),
    graph: syntheticGraph(),
    surfaces: syntheticSurfaces(),
    docsManifest: syntheticDocsManifest(),
    ...overrides,
  };
}

function sampleOf(example, overrides = {}) {
  return buildHoverSample(example, syntheticInputs(overrides));
}

test('buildHoverSample resolves a clean synthetic example', () => {
  const result = sampleOf(syntheticExample());
  assert.equal(result.ok, true, result.errors.join('; '));
  assert.deepEqual(result.errors, []);
  assert.equal(result.excerpt.text, SYNTHETIC_SOURCE);
  assert.equal(result.annotations.length, 2);

  const book = result.annotations.find((annotation) => annotation.key === 'book');
  assert.ok(book, 'the Book annotation is missing');
  assert.deepEqual(book, {
    key: 'book',
    name: 'Book',
    nodeId: 'class:demo.mox#demo.Book',
    nodeKind: 'class',
    kindLabel: 'class',
    signature: 'class Book',
    summary: 'A book in the demo catalogue.',
    docsUrl: 'https://docs.pherolang.com/reference/language/',
    docsLabel: 'Domain model reference',
    docsSource: 'docs/LANGUAGE.md',
    spanStart: SYNTHETIC_SOURCE.indexOf('Book'),
    spanEnd: SYNTHETIC_BOOK_END,
    targetId: 'sym-demo-book-book',
    descriptionId: 'desc-demo-book-book',
  });

  const borrow = result.annotations.find((annotation) => annotation.key === 'borrow');
  assert.ok(borrow, 'the borrow annotation is missing');
  assert.equal(borrow.nodeKind, 'operation');
  assert.equal(borrow.kindLabel, 'operation');
  assert.equal(borrow.spanStart, SYNTHETIC_BORROW_START);
  assert.equal(borrow.spanEnd, SYNTHETIC_BORROW_END);
  assert.equal(borrow.docsUrl, 'https://docs.pherolang.com/reference/language/');
});

/* ------------------------------------------------------------------ */
/* buildHoverSample — failure modes                                    */
/* ------------------------------------------------------------------ */

function withSymbols(...symbols) {
  return { ...syntheticExample(), symbols };
}

function symbolAt(index) {
  return { ...syntheticExample().symbols[index] };
}

test('buildHoverSample rejects a graph node that does not exist', () => {
  const example = withSymbols(
    { ...symbolAt(0), node: 'class:demo.mox#demo.Nope' },
    symbolAt(1),
  );
  const result = sampleOf(example);
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /graph node "class:demo\.mox#demo\.Nope" does not exist/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample rejects a graph node whose short name disagrees with the symbol', () => {
  const graph = syntheticGraph();
  graph.nodes[0].name = 'demo.Booklet';
  const result = sampleOf(syntheticExample(), { graph });
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /graph node is named "demo\.Booklet", not "Book"/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample rejects a signature the excerpt does not contain', () => {
  const example = withSymbols(
    { ...symbolAt(0), signature: 'class Library' },
    symbolAt(1),
  );
  const result = sampleOf(example);
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /signature "class Library" is not in the excerpt/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample rejects a symbol with no matching span of its classes', () => {
  const example = withSymbols(
    { ...symbolAt(0), classes: ['annotation'] },
    symbolAt(1),
  );
  const result = sampleOf(example);
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /expected exactly one annotation span for "Book", found 0/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample rejects an excerpt whose surface differs from the example', () => {
  const result = sampleOf(syntheticExample(), { excerpt: syntheticExcerpt('ddd') });
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /excerpt "demo" is ddd, expected mox/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample rejects a graph node with no documentation page', () => {
  const result = sampleOf(syntheticExample(), { docsManifest: { generated: [] } });
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /no docs page for graph node file "support\/support\.mox"/.test(error)),
    result.errors.join('; '),
  );
  // The annotations still resolve; only the link is absent.
  assert.equal(result.annotations.length, 2);
  assert.equal(result.annotations[0].docsUrl, null);
  assert.equal(result.annotations[0].docsSource, null);
});

test('buildHoverSample rejects a sample with no capability/operation symbol', () => {
  const result = sampleOf(withSymbols(symbolAt(0)));
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /no capability\/operation node is annotated/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample rejects a sample with no class/type symbol', () => {
  const result = sampleOf(withSymbols(symbolAt(1)));
  assert.equal(result.ok, false);
  assert.ok(
    result.errors.some((error) => /no class\/type node is annotated/.test(error)),
    result.errors.join('; '),
  );
});

test('buildHoverSample falls back on a null or empty artifact', () => {
  for (const artifact of [null, { formatVersion: 1, entries: {} }]) {
    const result = sampleOf(syntheticExample(), { artifact });
    assert.equal(result.ok, false);
    assert.ok(
      result.errors.some((error) => /no highlight-artifact entry for "demo"/.test(error)),
      result.errors.join('; '),
    );
    assert.deepEqual(result.annotations, []);
  }
});

/* ------------------------------------------------------------------ */
/* validateHoverIndex — the committed index                            */
/* ------------------------------------------------------------------ */

const committedIndex = loadHoverIndex(REPO);
const committedArtifact = loadArtifact(REPO);
const committedExcerpts = new Map(
  allExcerptSpecs(REPO).map((spec) => [spec.key, loadExcerpt(spec.key, REPO)]),
);
const committedGraph = readJson(join(REPO, 'site', 'data', 'graph.json'));
const committedSurfaces = readJson(join(REPO, 'site', 'data', 'surfaces.json'));
const committedDocsManifest = readJson(join(REPO, 'site', 'data', 'docs-manifest.json'));

test('the committed highlight artifact loads for the hover index', () => {
  assert.equal(committedArtifact.error, null);
  assert.ok(committedArtifact.artifact, committedArtifact.error ?? 'the artifact did not load');
});

test('validateHoverIndex accepts the committed hover.json', () => {
  const result = validateHoverIndex(committedIndex, {
    excerpts: committedExcerpts,
    artifact: committedArtifact.artifact,
    graph: committedGraph,
    surfaces: committedSurfaces,
    docsManifest: committedDocsManifest,
  });
  assert.deepEqual(result.problems, []);
  assert.equal(result.ok, true);
  assert.ok(result.samples.length > 0, 'the index resolved to no samples');
  for (const sample of result.samples) {
    assert.ok(sample.excerpt, `${sample.example.id}: no excerpt resolved`);
    assert.ok(sample.annotations.length > 0, `${sample.example.id}: no annotations resolved`);
    for (const annotation of sample.annotations) {
      assert.ok(annotation.docsUrl, `${sample.example.id}/${annotation.key}: no docs URL`);
      assert.match(annotation.docsUrl, /^https:\/\/docs\.pherolang\.com\//);
    }
  }
});

test('the committed index annotates both a class/type and a capability/operation', () => {
  const result = validateHoverIndex(committedIndex, {
    excerpts: committedExcerpts,
    artifact: committedArtifact.artifact,
    graph: committedGraph,
    surfaces: committedSurfaces,
    docsManifest: committedDocsManifest,
  });
  const kinds = new Set(result.samples.flatMap((sample) => sample.annotations.map((a) => a.nodeKind)));
  assert.ok([...kinds].some((kind) => HOVER_TYPE_KINDS.has(kind)), `no class/type kind in ${[...kinds]}`);
  assert.ok(
    [...kinds].some((kind) => HOVER_CALLABLE_KINDS.has(kind)),
    `no capability/operation kind in ${[...kinds]}`,
  );
});
