/**
 * The small amount of HTML reading the static checks need.
 *
 * Deliberately a scanner, not a parser: the anchor and script inventories are
 * flat attribute lookups on Astro/Starlight output, and the one check that
 * genuinely needs a DOM (`a11y`) already runs a real browser. Keeping this
 * dependency-free means `links`, `budget` and `coverage` stay sub-second and
 * cannot silently start interpreting HTML differently from the browser.
 *
 * Attribute values are decoded for the entity references Astro emits inside
 * `href` (`&amp;` in query strings, `&#39;` in a few `alt`s). IDs are compared
 * verbatim — an anchor is only real if the byte sequence matches, which is
 * exactly what the browser does after decoding.
 */

const TAG = (name) => new RegExp(`<${name}\\b([^>]*)>`, 'gi');
const ATTR = /([a-zA-Z_:][-a-zA-Z0-9_:.]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))/g;

/** Decode the five entities that can appear in an attribute value we care about. */
export function decodeEntities(value) {
  return value
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&amp;/g, '&');
}

/** Parse the attributes of one tag's inner text. */
export function attributes(inner) {
  const out = {};
  for (const match of inner.matchAll(ATTR)) {
    const value = match[2] ?? match[3] ?? match[4] ?? '';
    out[match[1].toLowerCase()] = decodeEntities(value);
  }
  return out;
}

/** Every `<a …>` in the document, in source order. */
export function anchors(html) {
  const out = [];
  for (const match of html.matchAll(TAG('a'))) {
    const attrs = attributes(match[1]);
    if (typeof attrs.href === 'string') out.push(attrs.href);
  }
  return out;
}

/** Every `<script …>` in the document. */
export function scripts(html) {
  const out = [];
  for (const match of html.matchAll(TAG('script'))) out.push(attributes(match[1]));
  return out;
}

/** Every `id` in the document, verbatim. */
export function ids(html) {
  const out = new Set();
  for (const match of html.matchAll(/\sid\s*=\s*(?:"([^"]*)"|'([^']*)')/g)) {
    out.add(match[1] ?? match[2] ?? '');
  }
  return out;
}
