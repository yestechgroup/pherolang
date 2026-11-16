/**
 * Span-to-HTML rendering shared by both origins.
 *
 * The artifact carries **byte** offsets (docs/HIGHLIGHTING.md); JavaScript
 * strings are indexed in UTF-16 code units. Every function here converts once
 * through a byte -> code-unit map, so non-ASCII source cannot shift a span.
 *
 * This module is deliberately pure: no filesystem, no artifact knowledge.
 */

import { classNamesFor } from './taxonomy.mjs';

const HTML_ESCAPES = new Map([
  ['&', '&amp;'],
  ['<', '&lt;'],
  ['>', '&gt;'],
  ['"', '&quot;'],
  ["'", '&#39;'],
]);

/** Escape text for element content (the artifact never reaches an attribute). */
export function escapeHtml(text) {
  return String(text).replace(/[&<>"']/g, (character) => HTML_ESCAPES.get(character));
}

/** UTF-8 byte length of a JS string. */
export function utf8Length(text) {
  return Buffer.byteLength(text, 'utf8');
}

/**
 * Build the byte -> UTF-16 index map for one source string.
 *
 * `map[byteOffset]` is the code-unit index at that byte boundary. Every offset
 * the compiler emits is a UTF-8 boundary, so the array has an entry for each.
 */
export function buildByteMap(source) {
  const byteLength = utf8Length(source);
  const map = new Uint32Array(byteLength + 1);
  let byte = 0;
  let index = 0;
  for (const character of source) {
    const length = utf8Length(character);
    for (let offset = 0; offset < length; offset += 1) map[byte + offset] = index;
    byte += length;
    index += character.length;
  }
  map[byteLength] = index;
  return map;
}

/**
 * Split one byte range into pieces carrying their class list.
 *
 * `pieces[i].start`/`end` are UTF-16 indices *relative to the range start*, so
 * the docs transformer can map them onto rendered text nodes. Gaps (whitespace
 * and other unclassified bytes) are explicit pieces with `classNames === ''`.
 */
export function piecesForRange(source, spans, byteMap, startByte, endByte, classNames = classNamesFor) {
  const base = byteMap[startByte] ?? 0;
  const end = byteMap[endByte] ?? base;
  const pieces = [];
  let cursor = startByte;

  const push = (fromByte, toByte, names) => {
    const from = (byteMap[fromByte] ?? base) - base;
    const to = (byteMap[toByte] ?? base) - base;
    if (to <= from) return;
    pieces.push({ start: from, end: to, classNames: names, text: source.slice(byteMap[fromByte], byteMap[toByte]) });
  };

  for (const span of spans ?? []) {
    if (span.end <= startByte || span.start >= endByte) continue;
    const from = Math.max(span.start, startByte);
    const to = Math.min(span.end, endByte);
    if (from > cursor) push(cursor, from, '');
    if (to > from) push(from, to, classNames(span).join(' '));
    cursor = to;
  }
  if (cursor < endByte) push(cursor, endByte, '');
  return pieces;
}

/**
 * Render one source range as an HTML fragment.
 *
 * The result is safe to insert with Astro's `set:html`: every text node is
 * escaped here. Newlines and whitespace are preserved exactly; no `<br>` is
 * introduced.
 */
export function renderInlineHtml(source, spans) {
  const text = String(source ?? '');
  if (text.length === 0) return '';

  const byteMap = buildByteMap(text);
  const pieces = piecesForRange(text, spans, byteMap, 0, utf8Length(text));
  let html = '';
  for (const piece of pieces) {
    if (piece.classNames === '') {
      html += escapeHtml(piece.text);
    } else {
      html += `<span class="${piece.classNames}">${escapeHtml(piece.text)}</span>`;
    }
  }
  return html;
}

/**
 * Render one source range as highlighted HTML, wrapping annotated spans in
 * links to their reference entries.
 *
 * This is the same span stream and the same `hl-*` classes as
 * {@link renderInlineHtml}; the only addition is an optional `<a>` around a
 * span whose byte `start` appears in `annotations`. Annotation identity is
 * the compiler's span start, so a stale index can never shift a link to a
 * different token: the caller must resolve the exact span first.
 *
 * `annotations` entries carry `spanStart`, `key`, `targetId`, and
 * `descriptionId`. Unknown/missing annotations are simply not linked.
 */
export function renderAnnotatedInlineHtml(source, spans, annotations = []) {
  const text = String(source ?? '');
  if (text.length === 0) return '';

  const byteMap = buildByteMap(text);
  const byStart = new Map();
  for (const annotation of annotations ?? []) {
    if (annotation && Number.isInteger(annotation.spanStart)) {
      byStart.set(annotation.spanStart, annotation);
    }
  }

  let html = '';
  let cursor = 0;
  for (const span of spans ?? []) {
    const from = byteMap[span.start];
    const to = byteMap[span.end];
    if (from === undefined || to === undefined || to <= from || from < cursor) continue;
    if (from > cursor) html += escapeHtml(text.slice(cursor, from));

    const classNames = classNamesFor(span).join(' ');
    const inner = text.slice(from, to);
    if (classNames === '') {
      html += escapeHtml(inner);
    } else {
      const rendered = `<span class="${classNames}">${escapeHtml(inner)}</span>`;
      const annotation = byStart.get(span.start);
      if (annotation) {
        html +=
          `<a class="pherolang-hover__symbol" ` +
          `href="#${escapeHtml(annotation.targetId)}" ` +
          `data-symbol="${escapeHtml(annotation.key)}" ` +
          `aria-describedby="${escapeHtml(annotation.descriptionId)}">` +
          rendered +
          '</a>';
      } else {
        html += rendered;
      }
    }
    cursor = to;
  }
  if (cursor < text.length) html += escapeHtml(text.slice(cursor));
  return html;
}
