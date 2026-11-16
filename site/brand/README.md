# Phero Lang brand assets

Starter identity for the web presence (epic #227, task EP1-02). Treat this as a
**direction to validate and refine**, not a finished identity. The mark is the
most provisional asset here; the token rules below are the part worth keeping.

Design direction, from the strategy (#226): *architectural precision* — Swiss
editorial design meets modern engineering software. Precise, minimal, confident,
technical.

## The mark

Two crossing strokes with four terminal nodes and an electric-lime core.

- The **crossing strokes** are two systems meeting at one shared semantic core —
  the product thesis: many surfaces, one model. The name says the same thing from
  the other side: the Greek *phero*, "to carry", names what the core does between
  them. The mark is reinterpreted, not redrawn — the geometry predates the name
  and survives it.
- The **square terminal nodes** are diagram nodes, not decoration; they are what
  makes the mark read as engineering rather than as a generic cross.
- The **lime core** is the model itself. It is the only colour in the mark, so
  the accent belongs to the idea rather than to the chrome.

The mark is built on a 64-unit grid with a 2-unit base: arms at 45°, stroke 6,
terminal nodes 8×8, core lozenge 24 wide. The compact mark keeps the same grid
and the same lozenge, drops the terminal nodes, and insets the arms to 8–56 at
stroke 8 — the weight that still reads at 16px.

### The lockup (W2-04)

The lockup wordmark is the single token `pherolang`. "Phero Lang" is two words
and stays in prose; set as a wordmark it reads long, and the two-word spelling
belongs to first mention and the descriptor, not to the mark. The token is also
the binary name, so the lockup can never disagree with the install line.

## Files

| File | Use |
|---|---|
| `pherolang-mark.svg` | Primary mark, dark ink. Use at 32px and above on light surfaces |
| `pherolang-mark-inverse.svg` | Primary mark, light ink. Use at 32px and above on graphite (and on photography) |
| `pherolang-mark-compact.svg` | Compact mark, dark ink. **16–31px only** — the terminal nodes are removed at this size because they turn to noise |
| `pherolang-mark-compact-inverse.svg` | Compact mark, light ink, 16–31px on dark |
| `pherolang-tile.svg` | 512×512 graphite tile with rounded corners — GitHub avatar, social profile, OG base |
| `pherolang-lockup.svg` | Horizontal lockup, mark + wordmark, light surfaces |
| `pherolang-lockup-inverse.svg` | Horizontal lockup, dark surfaces |
| `pherolang-palette.svg` | Palette reference sheet |
| `pherolang-social-card.svg` / `.png` | 1200×630 social preview — the GitHub repository card and the OG template |
| `favicons/` | `favicon-16.png`, `favicon-32.png`, `favicon.ico` (16+32+48), `favicon-192.png`, `favicon-512.png`, `apple-touch-icon.png` (180) |

The favicons are cut from the **compact** mark on the graphite tile, not from the
primary mark: at 16px the terminal nodes read as specks, and the tile keeps the
mark visible on a light tab bar and a dark one alike. `apple-touch-icon.png` is
full-bleed — iOS applies its own corner mask, so a transparent corner would fill
black.

The social card is 1200×630 on graphite: a charcoal tile holding the compact
mark, the outlined wordmark at 80px, and the descriptor set in Inter Regular 26px
at silver. Every pair on it is one the table below passes — warm white and silver
on graphite.

Always pair the name with its descriptor on first use in a page or profile:
**Phero Lang — Verifiable Model-Driven Engineering**. Never shorten the name in
prose: the product is two words, the wordmark and binary are `pherolang`, and the
crates are `phero-*`. The compound is the defensible name; the rules and the
reason live in [`docs/NAMING.md`](../../docs/NAMING.md).

## Wordmark

The lockup wordmark is **outlined** — paths, not text — so it renders identically
wherever a webfont is absent (a social card, an email, a slide).

It is Inter SemiBold (weight 600), set at 36px on a baseline of `y=44` with
−0.8px letter-spacing, converted to a single `<path>` (nine glyphs, thirteen
closed contours).
Advances and kern pairs come from the font, not from hand spacing; the wordmark
is not tracked by eye. Inter is SIL OFL 1.1, so the outline can ship. The lockup
`viewBox` is `0 0 274 64` — the mark on the 0–64 box, the wordmark origin at
`x=80`, and the trailing space matched to the optical gap after the mark. The
social card reuses the same outline at 80px, with the tracking scaled by the type
size (−1.78px at 80px).

Redraw the wordmark from a real Inter SemiBold file; do not re-space the paths.

## Palette

| Token | Hex | Role |
|---|---|---|
| Graphite | `#111318` | Primary background (dark surfaces) and body text (light surfaces) |
| Charcoal | `#1C2028` | Raised surface on graphite |
| Warm white | `#F5F4F0` | Reading surface, and text on dark |
| Silver | `#A7ADB7` | Borders, rules, diagram chrome — **decoration only on light surfaces** |
| Electric lime | `#C4F56A` | Accent: CTA fills, focus states, the model core, diagram highlights |
| Deep blue | `#5369D8` | Technical diagrams and model visualisation |

## Contrast rules (measured, not estimated)

Every value below is a real WCAG 2.2 relative-luminance ratio. These are
constraints on the design system, not suggestions — EP1-04 turns them into an
automated test.

| Pair | Ratio | Verdict |
|---|---|---|
| Graphite on warm white | 16.88 | AAA — body text |
| Warm white on charcoal | 14.83 | AAA — body text on dark |
| Lime on graphite | 14.70 | AAA — lime text **is** legal on dark |
| Graphite on lime | 14.70 | AAA — the CTA: lime fill, graphite label |
| Silver on graphite | 8.23 | AAA — muted text on dark |
| **Silver on warm white** | **2.05** | **FAIL** — never text on light. Borders and decoration only |
| **Lime on warm white** | **1.15** | **FAIL** — never text, never a thin icon on light |
| **Deep blue on warm white** | **4.36** | **FAIL** for body text (needs 4.5) — diagrams only |
| **Deep blue on graphite** | **3.88** | **FAIL** — diagrams only |

Three colours in the strategy's palette cannot carry text. The system needs
substitutes, and these are measured:

| Token | Hex | Ratio | Use |
|---|---|---|---|
| Muted text (light) | `#5A616B` | 5.68 on warm white | Secondary text on light surfaces — the legal replacement for silver |
| Link (light) | `#3F51B5` | 6.24 on warm white | Links and interactive text on light |
| Link (dark) | `#8B9AF5` | 7.11 on graphite | Links on dark surfaces |

Derive the rest of the ramp from these rather than inventing new hues; if a new
colour is needed, measure it before it enters `packages/tokens`.

### Code highlighting tokens

Code is coloured by a measured semantic set, not by the brand palette: the
`--code-*` tokens in `packages/tokens/src/tokens.css` map one-to-one onto the
shared `.hl-*` taxonomy, on `--code-bg` (`#FFFFFF` light, `#1C2028` dark).

| Token | Light | Dark |
|---|---|---|
| `--code-keyword` | `#3F51B5` | `#8B9AF5` |
| `--code-type` | `#00695C` | `#7FDBCA` |
| `--code-function` | `#8C4A00` | `#F4B860` |
| `--code-namespace` | `#5B3FA8` | `#C9A7F5` |
| `--code-variable` / `--code-parameter` / `--code-operator` / `--code-punctuation` | `#111318` | `#F5F4F0` |
| `--code-property` | `#1F4E79` | `#9CC7F0` |
| `--code-string` | `#2E6B2E` | `#A5D6A7` |
| `--code-number` | `#7A3E9D` | `#C9A7F5` |
| `--code-comment` | `#5A616B` | `#A7ADB7` |
| `--code-annotation` | `#8A1C5B` | `#F2A6CE` |
| `--code-constant` | `#6B4E00` | `#E6D27A` |
| `--code-escape` | `#8C4A00` | `#F4B860` |
| `--code-error` | `#B00020` | `#FF8A95` |

Every one of these is a pair in `packages/tokens/contrast.json` — light on
`#FFFFFF`, dark on `#1C2028`, all AA or better — and the gate fails any
`--code-*` declaration that no pair measures.

## Type

- **UI**: Geist, falling back to Inter. Self-host, subset, `font-display: swap`.
- **Code and technical labels**: IBM Plex Mono.
- **Editorial display**: one distinctive face for large headlines, licence
  verified before adoption. The wordmark is Inter SemiBold, outlined, so it no
  longer depends on the UI face being loaded.

Large headlines are the site's defining typographic characteristic. The
strategy's example is the standard to write to: *"Software is becoming easier to
generate. Making it understandable is the real engineering challenge."* — not
*"Welcome to Phero Lang, the next-generation multi-target DSL platform."*

## Provisional items

- **The wordmark is outlined** (see above), so the lockup no longer renders
  differently wherever Geist or Inter is absent. It needs no webfont.
- The mark has not been tested below 16px, in embroidery, or as a watermark. It
  does not need to be.
- **A favicon set exists** at `favicons/` — 16, 32, 180, 192 and 512 plus a
  multi-size `favicon.ico` — cut from the compact mark on the graphite tile. The
  16px file was checked in one colour at 100%; the 180px file is full-bleed for
  iOS's corner mask. It has not been checked on a physical device (Safari pinned
  tab, Android launcher mask), which is worth doing before launch.
- The social card is a template, not a generator: copy `pherolang-social-card.svg`
  and keep its grid rather than re-composing the layout by hand.
