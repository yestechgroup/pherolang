/**
 * html-validate configuration for the built Astro / Starlight output.
 *
 * Base preset: `html-validate:document` — the structural rules for a document
 * (doctype, one `<html>`/`<head>`/`<body>`, unique ids, valid nesting, label
 * rules, heading order). Not `:recommended`, and not the a11y preset: this
 * check owns *structural validity*, the `a11y` check owns rendered semantics
 * with a real browser, and duplicating axe's rules here would only produce two
 * different verdicts for one defect.
 *
 * Two named exceptions, each with the reason it exists — no rule is off
 * "because it is noisy":
 *
 *   * `require-sri` (both apps) — Subresource Integrity. Every script and
 *     stylesheet these sites serve is first-party, content-hashed and
 *     `immutable` (see `public/_headers` in each app), and
 *     `Content-Security-Policy: script-src 'self'` already forbids any other
 *     origin. SRI adds nothing against a threat that does not exist here, and
 *     cannot cover Starlight's dynamically imported Pagefind chunks at all.
 *
 *   * `heading-level`, `minInitialRank: "h2"` (docs app only) — Starlight
 *     renders a `<nav aria-labelledby="starlight__on-this-page">` whose `<h2>`
 *     is the landmark's own accessible name, and that nav is emitted before
 *     `<main>`. A labelled landmark heading is not a step in the document
 *     outline, but it *is* the first heading in source order, so the default
 *     "a document starts at `<h1>`" rule cannot hold for a page whose shell is
 *     third-party. Everything from that point on — one `<h1>`, no skipped
 *     levels — is still enforced, and both invariants are independently
 *     asserted per route in the `a11y` check. The marketing app, whose layout
 *     this project owns, keeps the strict `h1`-first rule.
 */

/** Rules shared by both apps: the document preset, one named exception. */
const COMMON = {
  'require-sri': 'off',
};

const HEADING = 'heading-level';

export function configFor(appKey) {
  return {
    root: true,
    extends: ['html-validate:document'],
    rules:
      appKey === 'docs'
        ? { ...COMMON, [HEADING]: ['error', { minInitialRank: 'h2' }] }
        : { ...COMMON },
  };
}

/** The marketing configuration, for callers that do not care about the split. */
export default configFor('web');
