/**
 * The stable highlighting vocabulary the site consumes.
 *
 * This is *not* a grammar and not a keyword list: the classes and modifiers
 * are the wire spellings `phero-highlight` already emits (docs/HIGHLIGHTING.md
 * owns the contract). The site needs the set to spell CSS classes, measure
 * contrast, and decide which fences belong to the highlighting pipeline.
 */

import { SURFACE_IDS } from '../../../scripts/surfaces.mjs';

/** Canonical class wire spellings, in the order the Rust taxonomy lists them. */
export const HIGHLIGHT_CLASSES = [
  'keyword',
  'type',
  'function',
  'namespace',
  'variable',
  'parameter',
  'property',
  'string',
  'number',
  'comment',
  'operator',
  'punctuation',
  'annotation',
  'constant',
  'escape',
  'error',
];

export const HIGHLIGHT_CLASS_SET = new Set(HIGHLIGHT_CLASSES);

/** Canonical modifier wire spellings (camelCase, as the CLI emits them). */
export const HIGHLIGHT_MODIFIERS = [
  'declaration',
  'definition',
  'readonly',
  'deprecated',
  'documentation',
  'defaultLibrary',
];

export const HIGHLIGHT_MODIFIER_SET = new Set(HIGHLIGHT_MODIFIERS);

/** The nine Phero Lang surfaces, from the site's one authored surface table. */
export const PHERO_SURFACE_IDS = [...SURFACE_IDS];
export const PHERO_SURFACE_SET = new Set(PHERO_SURFACE_IDS);

/** True when `language` names a Phero Lang surface. */
export function isPheroSurface(language) {
  return typeof language === 'string' && PHERO_SURFACE_SET.has(language);
}

/**
 * Fence languages that are deliberately left to Expressive Code/Shiki or to
 * plaintext. A docs fence outside the Phero surfaces and this list fails the
 * site gate, so adding a language is an explicit decision.
 */
export const EXEMPT_DOC_LANGS = new Set([
  '',
  'text',
  'txt',
  'plaintext',
  'markdown',
  'md',
  'mdx',
  'sh',
  'bash',
  'shell',
  'console',
  'rust',
  'rs',
  'json',
  'jsonc',
  'json5',
  'html',
  'css',
  'js',
  'javascript',
  'ts',
  'typescript',
  'yaml',
  'yml',
  'toml',
  'xml',
  'diff',
  'sql',
  'python',
  'go',
  'java',
  'c',
  'cpp',
  'csharp',
  'ruby',
  'php',
  'swift',
  'kotlin',
  'scala',
  'hcl',
  'dockerfile',
  'makefile',
  'ini',
  'csv',
  'log',
]);

/** camelCase modifier -> kebab-case class suffix (`defaultLibrary`). */
export function kebabModifier(modifier) {
  return modifier.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

/**
 * The HTML class list for one span. Unknown classes/modifiers are skipped so a
 * newer artifact stays readable; the reference Rust model rejects them instead
 * of guessing, and the gate pins `formatVersion` so this is a narrow fallback.
 */
export function classNamesFor(span) {
  const classes = [];
  if (HIGHLIGHT_CLASS_SET.has(span.class)) classes.push(`hl-${span.class}`);
  for (const modifier of span.modifiers ?? []) {
    if (HIGHLIGHT_MODIFIER_SET.has(modifier)) classes.push(`hl-${kebabModifier(modifier)}`);
  }
  return classes;
}
