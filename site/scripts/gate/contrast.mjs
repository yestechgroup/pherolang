/**
 * Check 4 — the colour system, measured rather than asserted.
 *
 * `packages/tokens/contrast.json` claims a WCAG 2.2 ratio for every pair the
 * design system actually uses, and `src/tokens.css` holds the values. This
 * check recomputes each ratio from the CSS-side hex values and cross-checks
 * the two files against each other, so neither can drift: a hex edited in the
 * stylesheet without re-measuring the pair, or a pair "measured" against a
 * colour the stylesheet no longer has, is a failure.
 *
 * The brand palette is not the only measured set: every declared `--code-*`
 * semantic token must have its own pair too, so a new highlighting colour
 * cannot enter the stylesheet unmeasured.
 *
 * The formula is WCAG 2.2 §1.4.3 / §1.4.6, implemented here so the check has
 * no dependency and no browser: sRGB channel -> linear light -> relative
 * luminance (0.2126 R + 0.7152 G + 0.0722 B) -> (Lmax + 0.05) / (Lmin + 0.05).
 */

import { TOKENS } from './paths.mjs';
import { plural, readJson, readText } from './util.mjs';

const NAME = 'contrast';

/** WCAG 2.2 thresholds for the two conformance levels the tokens claim. */
const LEVEL_THRESHOLD = { AA: 4.5, AAA: 7 };

const HEX = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;
const PALETTE_DECLARATION = /--palette-([a-z0-9-]+)\s*:\s*(#[0-9a-fA-F]{3,6})\s*;/g;
const CODE_DECLARATION = /--code-([a-z0-9-]+)\s*:\s*(#[0-9a-fA-F]{3,6})\s*;/g;
const ANY_HEX = /#[0-9a-fA-F]{3,8}\b/g;

/** Hex -> {r,g,b} in 0..255. Returns null when the literal is not a colour. */
function parseHex(value) {
  const match = HEX.exec(value.trim());
  if (!match) return null;
  const digits = match[1];
  const expand = (pair) => Number.parseInt(pair, 16);
  if (digits.length === 3) {
    return {
      r: expand(digits[0] + digits[0]),
      g: expand(digits[1] + digits[1]),
      b: expand(digits[2] + digits[2]),
    };
  }
  return {
    r: expand(digits.slice(0, 2)),
    g: expand(digits.slice(2, 4)),
    b: expand(digits.slice(4, 6)),
  };
}

/** sRGB channel (0..255) -> linear light (0..1). */
function linearize(channel) {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** WCAG relative luminance of an sRGB colour. */
function relativeLuminance({ r, g, b }) {
  return 0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b);
}

/** WCAG contrast ratio, always >= 1. */
export function contrastRatio(fg, bg) {
  const a = relativeLuminance(parseHex(fg));
  const b = relativeLuminance(parseHex(bg));
  const [light, dark] = a >= b ? [a, b] : [b, a];
  return (light + 0.05) / (dark + 0.05);
}

const normalise = (hex) => {
  const parsed = parseHex(hex);
  if (!parsed) return null;
  const part = (value) => value.toString(16).padStart(2, '0');
  return `${part(parsed.r)}${part(parsed.g)}${part(parsed.b)}`;
};

const fixed = (value) => value.toFixed(2);

export const check = {
  name: NAME,
  title: 'contrast.json ratios recompute and match tokens.css (WCAG 2.2)',

  run() {
    const lines = [];
    const problems = [];

    const tokens = readJson(TOKENS.contrast);
    const css = readText(TOKENS.css);

    // --- the stylesheet side -------------------------------------------
    const palette = new Map();
    for (const match of css.matchAll(PALETTE_DECLARATION)) {
      palette.set(match[1], match[2].toLowerCase());
    }
    if (palette.size === 0) {
      problems.push(`no --palette-* colour declarations found in ${TOKENS.css}`);
    }

    // Code colours are a second measured set: the `.hl-*` semantic tokens are
    // not palette entries, so each declaration must have its own pair.
    const code = new Map();
    for (const match of css.matchAll(CODE_DECLARATION)) {
      code.set(match[1], match[2].toLowerCase());
    }

    const cssHexes = new Set([...css.matchAll(ANY_HEX)].map((match) => normalise(match[0])).filter(Boolean));

    // --- the measured side ---------------------------------------------
    const measured = new Set();
    const pairs = [...(tokens.pairs ?? []), ...(tokens.forbidden ?? [])];
    if (pairs.length === 0) problems.push(`${TOKENS.contrast} contains no pairs`);

    for (const pair of tokens.pairs ?? []) {
      const fg = parseHex(pair.fg ?? '');
      const bg = parseHex(pair.bg ?? '');
      if (!fg || !bg) {
        problems.push(`${pair.name}: fg/bg is not a hex colour (${pair.fg} on ${pair.bg})`);
        continue;
      }
      measured.add(normalise(pair.fg));
      measured.add(normalise(pair.bg));

      const ratio = contrastRatio(pair.fg, pair.bg);
      const ok = ratio + 1e-6 >= pair.min;
      if (!ok) {
        problems.push(
          `${pair.name}: ${fixed(ratio)}:1 is below the required ${pair.min}:1 (${pair.fg} on ${pair.bg})`,
        );
      }

      const claimed = LEVEL_THRESHOLD[pair.verdict];
      if (claimed !== undefined && pair.min + 1e-6 < claimed) {
        problems.push(
          `${pair.name}: claims ${pair.verdict} (>= ${claimed}:1) but pins min ${pair.min}:1 — a claimed level cannot be looser than the level`,
        );
      }

      lines.push(
        `     ${ok ? 'ok  ' : 'FAIL'} ${pair.name.padEnd(32)} ${fixed(ratio).padStart(6)}:1  >= ${String(pair.min).padEnd(4)} ${pair.verdict ?? ''}`.trimEnd(),
      );
    }

    for (const pair of tokens.forbidden ?? []) {
      if (!parseHex(pair.fg ?? '') || !parseHex(pair.bg ?? '')) {
        problems.push(`${pair.name}: fg/bg is not a hex colour (${pair.fg} on ${pair.bg})`);
        continue;
      }
      measured.add(normalise(pair.fg));
      measured.add(normalise(pair.bg));

      const ratio = contrastRatio(pair.fg, pair.bg);
      const ok = ratio <= pair.max + 1e-6;
      if (!ok) {
        problems.push(
          `${pair.name}: ${fixed(ratio)}:1 exceeds the forbidden ceiling ${pair.max}:1 (${pair.fg} on ${pair.bg}) — ${pair.why ?? 'no reason recorded'}`,
        );
      }
      lines.push(
        `     ${ok ? 'ok  ' : 'FAIL'} ${pair.name.padEnd(32)} ${fixed(ratio).padStart(6)}:1  <= ${String(pair.max).padEnd(4)} forbidden`.trimEnd(),
      );
    }

    // --- the two files must describe the same palette --------------------
    for (const [name, hex] of [...palette].sort()) {
      const key = normalise(hex);
      if (!measured.has(key)) {
        problems.push(
          `--palette-${name}: ${hex} is declared in tokens.css but no pair in contrast.json measures it`,
        );
      }
    }
    for (const hex of [...measured].sort()) {
      if (!cssHexes.has(hex)) {
        problems.push(
          `contrast.json measures a colour the stylesheet does not declare: ${hex} (looked for it anywhere in tokens.css)`,
        );
      }
    }

    // --- every declared code colour must be measured --------------------
    for (const [name, hex] of [...code].sort()) {
      if (!measured.has(normalise(hex))) {
        problems.push(
          `--code-${name}: ${hex} is declared in tokens.css but no pair in contrast.json measures it`,
        );
      }
    }
    lines.push(
      `     code: ${plural(
        [...code.keys()].filter((name) => name !== 'bg').length,
        'code colour',
      )} plus --code-bg, each measured for light and dark`,
    );

    // Values outside the palette block are still worth seeing, but they are
    // surfaces (raised/sunken) rather than the palette this contract pins.
    const unmeasured = [...cssHexes].filter((hex) => !measured.has(hex)).sort();
    if (unmeasured.length > 0) {
      lines.push(
        `     note: ${plural(unmeasured.length, 'non-palette hex value')} in tokens.css is not a measured pair: ${unmeasured.join(', ')}`,
      );
    }

    lines.splice(
      0,
      0,
      `measured ${plural(tokens.pairs?.length ?? 0, 'pair')} (min) and ${plural(tokens.forbidden?.length ?? 0, 'pair')} (forbidden) over ${plural(palette.size, 'palette colour')}`,
    );

    if (problems.length > 0) {
      lines.push(`FAIL ${plural(problems.length, 'contrast problem')}:`);
      for (const problem of problems) lines.push(`     ${problem}`);
    } else {
      lines.push('ok   every pair meets its threshold and both files describe the same palette');
    }

    return { status: problems.length > 0 ? 'fail' : 'pass', lines };
  },
};
