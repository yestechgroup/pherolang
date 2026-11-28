# 0013 — Rune keeps its own token enum inside `phero-syntax`

- **Status:** Accepted, 2026-11-28
- **Issue:** pherolang#400 (RN9, epic pherolang#391)
- **Amends** [0005](0005-shared-surface-kernel-layering.md) D3 ("one lexer,
  one token table") **for the Rune surface only**. ADR 0005 is otherwise
  unchanged, and the nine `.mox`-family surfaces keep the one shared `Token`.
- **Follows** [0012](0012-rune-is-an-in-tree-surface.md) D2: the Rune grammar
  lives in `phero-syntax` (`phero_syntax::rune`).

## Context

RN9 moves the Rune parser out of the scannerless `sigil-syntax` and into
`phero-syntax` on a logos lexer and the shared `phero_parser::Tokens` input.
Rune's lexical rules do not fit the shared `.mox`-family token table:

- **Hyphenated keywords.** `to-date-time`, `to-zoned-date-time`, `one-of`,
  `only-element`, `post-condition` and others are single keyword tokens in
  Rune's Xtext grammar. In the shared lexer they would lex as
  `Ident Other('-') Ident`, and adding them as tokens there would change how
  `a-b` lexes in nine other surfaces.
- **Operators and numbers.** `->>` would lex as `Arrow` plus `Other('>')`.
  Decimals (`1.5`, `.5`) must coexist with `0..1` cardinality ranges, and the
  shared lexer has no decimal token.
- **Words.** Rune reserves no identifiers: CDM uses `condition`, `source`,
  `scope` and `version` as attribute names. Many Rune words (`type`, `enum`,
  `extends`, `annotation`, `version`, `as`, `import`, `when`) are reserved
  keyword tokens in the shared lexer.
- **Strings.** Rune strings may be single-quoted and may span lines.

Fitting Rune into the shared table would mean changing lexing for every other
surface, whose spans and messages are byte-pinned by goldens, or adding
surface-conditional lexing to one table, which is worse than two tables.

## Decision

`phero_syntax::rune::lexer::RuneToken` is a second logos token enum, in the
same crate. It follows the phero-ifml precedent: a surface-specific token type
over the shared `phero_parser::Tokens<'src, T>` input, which only needs
`T: Clone`. The constraints that keep this from becoming a parallel kernel:

- **Same crate and substrate.** It lives in `phero-syntax` and uses
  `phero_parser::{Tokens, Span, ParseError}` and the shared combinators. There
  is no separate span or error type.
- **Words are registered once.** Every word the Rune grammar matches is in
  `phero_syntax::words` (`RUNE_CONTEXTUAL`, `RUNE_HYPHEN_KEYWORDS`,
  `RUNE_TIMING_WORDS` and the stop-list tables). A drift test asserts the
  tables and the parser's words are exactly equal, and a second test checks
  the module-doc EBNF against them in both directions. Highlighting (RN16)
  and completion (RN15) read these tables; they never keep their own lists
  (ADR 0003).
- **No other surface may use `RuneToken`, and Rune may not use `Token`.**
  A third enum needs its own ADR.

## Consequences

- The shared `Token` and the other nine surfaces' lexing are unchanged.
- `phero-highlight` will need a third `LexerKind` when Rune is registered
  (RN16), alongside `MoxFamily` and `Ifml`.
- The token lexer fixed a latent bug in the scannerless parser. A bare
  left-parameter-less `one-of` or `to-string` was read as subtraction
  (`one - of`), which gave bogus "unknown symbol" errors (49 in the CDM
  corpus). Xtext lexes both as keywords (`Rosetta.xtext` line 578), and so
  does `RuneToken`.
