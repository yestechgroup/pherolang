# Compiler-backed syntax highlighting

Phero Lang source is highlighted by the compiler, not by a hand-written
grammar. One language model — the taxonomy, the nine-surface registry, and
the engine in the `phero-highlight` crate — drives every consumer: the
`pherolang highlight` JSON/HTML output in this slice, and later the static
sites (SH4), generated TextMate/VS Code grammars (SH5), and LSP semantic
tokens and hover (SH6).

The parser modules are the grammar authority. This document is the
**highlighting contract**: the vocabulary, the spans, the engine guarantees,
and the wire/HTML formats. When prose here and a lexer or parser disagree,
the lexer/parser wins; change both in the same commit.

> **Status.** This slice of epic SH (#286) covers the metadata registry
> (SH1, #287), the engine (SH2, #288), and the CLI/wire format (SH3, #289).
> Site integration, editor grammars, LSP tokens, and the interactive editor
> are **not** implemented here; see [Out of scope](#out-of-scope).

## Purpose

- **One model, every renderer.** The static site, the generated TextMate
  grammar, and LSP semantic tokens must all derive from the same span
  stream. There is no second taxonomy and no second grammar.
- **The compiler is the source of truth.** Keywords, contextual words,
  literals, built-in types, declarations, and references come from the
  lexers and ASTs in `phero-syntax` / `phero-ifml`. A site-side keyword list
  would be a second source of truth and is forbidden.
- **Never a second parser.** `phero-highlight` consumes tokens and ASTs; it
  does not lex, parse, or re-implement a grammar.
- **Always useful, never fatal.** Fragments, docs fences, and broken source
  receive best-effort lexical highlighting and an explicit `degraded` flag
  instead of an error page.
- **Deterministic and offline.** The same input produces byte-identical
  output; the CLI touches only its inputs and outputs, and never the
  network.

## Architecture

```text
phero-syntax (logos lexers + ASTs) ─┐
                                    ├─> phero-highlight
phero-ifml   (logos lexer + AST)  ──┘   taxonomy + registry + engine
                                        JSON / HTML renderers
                                             │
                        ┌────────────────────┼────────────────────┐
                        ▼                    ▼                    ▼
                pherolang highlight     site (SH4)          LSP (SH6)
                 JSON / HTML            TextMate (SH5)
```

- `phero-syntax` owns the `.mox` family lexer, parser, contextual word
  tables, and byte spans. `phero-ifml` owns the same for `.ifml`.
- `phero-highlight` owns `HighlightClass`, `HighlightModifier`,
  `HighlightSpan`, `HighlightOptions`/`HighlightMode`, `HighlightResult`,
  the nine-surface registry, the classifier, and the JSON/HTML renderers.
  It depends on the two front-end crates; they never depend on it.
- `phero-cli` hosts the command. Its output is the contract the site and
  editor tooling consume — they never link the Rust AST directly.
- The registry is the only place a consumer asks *what words does this
  surface know?* or *what does this token mean?*.

## Class taxonomy

`HighlightClass` and `HighlightModifier` are `#[non_exhaustive]`: a new
member is a deliberate additive change. A consumer that wants forward
compatibility should skip a span whose class it does not know; the reference
Rust model rejects an unknown class rather than guessing at a colour. The
`formatVersion` gate is the primary compatibility boundary. Class wire
spellings are **lowercase** (`keyword`, not `Keyword` or `KEYWORD`); HTML
classes prefix them with `hl-`.

| Wire spelling | Rust variant | Meaning |
|---|---|---|
| `keyword` | `Keyword` | A real lexer keyword, or a contextual word in the grammar position that makes it special |
| `type` | `Type` | A declared type name, a type reference, or a built-in type (built-ins also carry `defaultLibrary`) |
| `function` | `Function` | A function, operation, service operation, action, command, or capability name |
| `namespace` | `Namespace` | A package, application, module, import alias, or model-level namespace |
| `variable` | `Variable` | An ordinary value binding or reference (for example a `.ddd` statement binding) |
| `parameter` | `Parameter` | An operation/service parameter, `.ifml` param, command argument or option, or task input |
| `property` | `Property` | A feature, attribute, field, property assignment, or setting-path member |
| `string` | `String` | A string literal; escapes inside it are separate `escape` spans |
| `number` | `Number` | An integer or floating-point literal |
| `comment` | `Comment` | A line or block comment; documentation comments also carry `documentation` |
| `operator` | `Operator` | An expression or structural operator (`=`, `->`, `==`, `->`, …) |
| `punctuation` | `Punctuation` | A structural delimiter (braces, brackets, parentheses, commas, semicolons, colons, dots) |
| `annotation` | `Annotation` | An annotation name, decorator word, design flag, or their use positions |
| `constant` | `Constant` | A boolean, enum member, vocabulary word, ranking literal, or other closed-set literal |
| `escape` | `Escape` | An escape sequence inside a string literal; a surface that lexes the literal atomically reports it as part of `string` |
| `error` | `Error` | An invalid or unrecognized region; still bounded and non-overlapping |

Classes track the LSP semantic-token vocabulary closely so SH6 is a mapping,
not a second taxonomy. A surface that has no construct for a class simply
never emits it; that is not an error.

## Modifiers

`HighlightModifier` is a set on each span. Wire spellings are lower
camelCase (`defaultLibrary`); HTML spells them kebab-case
(`hl-default-library`). The array is always present and is serialized in the
canonical order below, without duplicates.

| Wire spelling | Rust variant | Meaning |
|---|---|---|
| `declaration` | `Declaration` | The identifier introduces a declaration |
| `definition` | `Definition` | The span is a symbol's definition site (LSP `definition` modifier) |
| `readonly` | `Readonly` | The declaration or binding is immutable (`readonly`, a `let` binding) |
| `deprecated` | `Deprecated` | The symbol is deprecated |
| `documentation` | `Documentation` | A documentation-carrying comment or node |
| `defaultLibrary` | `DefaultLibrary` | A built-in type or otherwise library-defined symbol |

Modifiers refine a class; they never replace it. An unknown modifier from a
newer format version is treated like an unknown class (skipped by a
forward-compatible consumer, rejected by the reference Rust model).

## The nine-surface registry

Surface ids are the file extensions and the canonical `--surface` values.
They mirror `phero_driver::SurfaceKind` — the compiler's one extension
table — and are drift-tested against it; the registry is the highlighter's
dispatch key, not a second extension map.

| Id | Extension | Lexer family | Language documentation |
|---|---|---|---|
| `mox` | `.mox` | `phero-syntax` (logos lexer + chumsky AST) | [LANGUAGE.md](LANGUAGE.md) |
| `ifml` | `.ifml` | `phero-ifml` (logos lexer + chumsky AST) | [IFML.md](IFML.md) |
| `ddd` | `.ddd` | `phero-syntax` | [DDD.md](DDD.md) |
| `actor` | `.actor` | `phero-syntax` | [LANGUAGE.md](LANGUAGE.md#actor-policy-files-actor) |
| `evt` | `.evt` | `phero-syntax` | [EVENTS.md](EVENTS.md) |
| `deploy` | `.deploy` | `phero-syntax` | [DEPLOY.md](DEPLOY.md) |
| `uc` | `.uc` | `phero-syntax` | [USECASES.md](USECASES.md) |
| `cmmn` | `.cmmn` | `phero-syntax` | [CMMN.md](CMMN.md) |
| `dmn` | `.dmn` | `phero-syntax` | [DMN.md](DMN.md) |

Each registry entry records the id, extension, display name, aliases, lexer
entry point, token/word classification, declaration forms, and the
documentation link. The keyword, contextual, and literal tables are
borrowed from `phero_syntax::words` / `phero_ifml::words`; built-in type
names come from `phero_ifml::words::TYPE_REFS` for `.ifml` and the shared
built-in list for the other surfaces. No table is copied into the
highlighter. Ids and extensions are canonical; aliases (`domain`, `flow`,
`design`, …) are editor convenience spellings, never a second taxonomy. A
surface with no construct for a class records that explicitly; a silent
omission is a bug.

## Source-of-truth rules

The grammar vocabulary has three owners, and no consumer keeps a fourth
copy. Built-in type names are the one registry-owned list.

### Real lexer keywords

For the `phero-syntax` family, the logos `#[token]` / `#[regex]`
declarations in `crates/phero-syntax/src/lexer.rs` are the lexical source.
`phero_syntax::words` exposes the derived tables — `KEYWORDS`,
`MOX_KEYWORDS`, `ACTOR_KEYWORDS`, `DDD_KEYWORDS`, `EVT_KEYWORDS`,
`DEPLOY_KEYWORDS`, `UC_KEYWORDS`, `CMMN_KEYWORDS`, `DMN_KEYWORDS`, and the
literal tables — and its tests fail when a table and the lexer drift apart.
The highlighter classifies through the registry; it never re-types the list.

A word is not a keyword on every surface. `container` is a real `.mox`
lexer keyword but an ordinary word in `.deploy` value position; `.deploy`
has only `import` as a real keyword, while `.ddd` has `import`,
`capability`, `when`, and `version`.

### Contextual words

Contextual words are ordinary identifiers that are special only in a grammar
position. Their authority is the parser module and the per-surface word
table in `phero-syntax` (`crates/phero-syntax/src/{ddd,deploy,dmn,usecases,
cmmn}.rs`), exposed through `phero_syntax::words` as the
`*_CONTEXTUAL` / literal tables. `id` and `readonly` are contextual feature
modifiers, not keywords; `schema` and `sigil` are contextual only after
`import`; `to` is contextual on a delegation line.

### `.ifml` curated words

`.ifml` has **no keyword tokens**: every word is an identifier and is
special only in the position the parser dispatches on. Its grammar
vocabulary is curated in `phero_ifml::words` (`STRUCTURAL_WORDS`,
`TYPE_REFS`, `BOOLEANS`) and kept aligned with the parser EBNF by tests.
The registry classifies `STRUCTURAL_WORDS` as `keyword` — they are grammar
words, not reserved tokens — and `TYPE_REFS` as `type` with
`defaultLibrary`. The closed data vocabularies (`button`, `list`, `text`, …)
remain owned by `phero-ir`; they are property values, not grammar words, and
are not highlighted as keywords.

### Grammar authority

- `.mox` family EBNF: [`crates/phero-syntax/src/parser/mod.rs`](../crates/phero-syntax/src/parser/mod.rs)
- `.ifml` EBNF: [`crates/phero-ifml/src/parser/mod.rs`](../crates/phero-ifml/src/parser/mod.rs)
- Expressions: [EXPRESSIONS.md](EXPRESSIONS.md)

### Adding a keyword

1. Change the owner: a logos `#[token]` for a real keyword, or the parser's
   contextual table for a contextual word.
2. Update the matching `phero_syntax::words` / `phero_ifml::words` table and
   its parser spot-check test.
3. Extend the docs-fence corpus and the affected golden. The registry and
   the highlighter need no edit — that is the point.

A new token **variant** is the exception: it joins the registry's exhaustive
classifier in the same commit, or the build fails.

### Adding a surface

1. Add the `SurfaceKind` arm and `SurfaceKind::ALL` entry (the compiler's
   extension table).
2. Add the lexer/parser and its word tables in the owning front-end crate.
3. Add one registry row (id, extension, lexer family, docs link) and a
   golden per renderer.
4. Extend the drift tests: every surface must have a registry entry, and
   no contextual word may leak between surfaces.

## Span contract

The core is byte-based and never panics on malformed input.

- A span is `{ start, end }` with **byte offsets** and a **half-open**
  range `[start, end)`. `start < end`; empty spans are never emitted.
- Spans are sorted by `start`, non-overlapping, and bounded by
  `0..=source.len()`. Every offset is a UTF-8 boundary because it comes
  from a lexer or parser span.
- Whitespace and other unclassified gaps are **implicit**: they are not
  emitted and are reconstructed from the source between spans. The v1 wire
  format never emits a neutral/whitespace span.
- Comments are atomic: a comment gets exactly one span and no semantic
  span may split it. A string literal is lexically segmented into `string`
  pieces plus `escape` sub-spans for each `\\.` sequence; no semantic span
  may split either region. In `.ifml`, a string that names a declaration or
  reference is a semantic span over the **whole literal**, quotes included;
  it replaces the lexical string/escape spans rather than overlapping them.
- Parser spans override lexical identifier/type guesses over the same
  range. Adjacent spans with the same class and modifier set are merged.
- `sourceHash` and all spans refer to the exact bytes supplied. No line
  ending is normalized and no text is rewritten.
- UTF-16 is a boundary concern only: the LSP adapter converts once
  (`to_utf16`) when it speaks LSP. The CLI and HTML renderers stay in
  bytes and slice the source.

Invariants are enforced by tests: sorted, non-overlapping, in bounds, no
empty spans, no semantic split of a comment/string, and no span outside the
source.

## Engine contract

```rust
highlight(source: &str, surface: Surface, options: HighlightOptions) -> HighlightResult
```

`HighlightResult` carries `spans`, `degraded`, and `diagnostics` (the last
for LSP/tests; the CLI does not print them as errors).

### Strict and lenient

- **Strict** is the default for files and directories. It runs the surface's
  parser, uses the recovered AST (partial or complete), records every parser
  and lexical diagnostic, and sets `degraded = true` when the source did not
  parse cleanly. It still returns a result — strictness never changes the
  never-fail guarantee.
- **Lenient** is the default for stdin and docs fences. It runs the same
  parser and the same recovered-AST walk, but suppresses the diagnostics and
  still sets `degraded` truthfully. It never fails and never turns a parse
  error into an error page.
- A **degraded** result is a successful result: exit code 0 and a valid
  artifact (JSON carries `"degraded": true`; HTML still renders the lexical
  spans). An empty source is not degraded; it has no spans.
- `degraded` is truthful. Intentional lexical regions (below) do not set it;
  a missing or partial semantic pass does.

### One lex, one parse

The engine calls the surface's existing lexer and parser exactly once per
input. It must not reparse per token, allocate a second copy of the source,
or introduce a second grammar. `.mox` uses `lex_with_comments_lossy`
(comments are part of the span stream and invalid regions never abort the
scan); `.ifml` uses `parse_ifml_syntax_recovering` (the same parser the
compiler uses, without import or domain resolution), never a new parser.

### Semantic coverage

The semantic pass walks the AST and classifies:

- declaration names (`class`, `enum`, `actor`, `capability`, `service`,
  `repository`, `event`, `channel`, `subscription`, `case`, `stage`, `task`,
  `decision`, `decision_model`, `view`, `component`, `action`, `usecase`,
  `application`, `module`, `profile`, `deployment`, …)
- type references, enum members, namespaces, packages, and import aliases
- operation/service names, parameters, features, fields, and task inputs
- annotations and their arguments where spans exist
- contextual keywords the parser recognized (`.ddd` `service`, `.dmn`
  `logic`, `.uc` `realizes`, `.ifml` `view`/`type`, …)

### Intentionally lexical nodes

These are lexical by design in this slice and are documented, not guessed:

- `.ddd` `expr` and `rust` bodies (the driver type-checks/executes them; the
  highlighter emits their raw text lexically)
- expression subnodes beyond declarations and references (a later
  refinement; SH2 prioritized declarations, references, types, functions,
  namespaces, and contextual keywords)
- comments, string literals, numbers, and error regions, which are lexical
  classes by definition

Leaving a node lexical is not a parse failure and does not set `degraded`.

### Resolution order

1. Comments and string interiors are immutable.
2. Parser declaration/reference spans override lexical guesses.
3. Adjacent equal spans merge.
4. Everything else falls back to the lexical class from the registry.

## JSON wire format (v1)

The JSON artifact is the contract. It is a versioned, deterministic,
camelCase document with no source text, paths, timestamps, or
environment-specific data. For the source

```mox
package shop

class Order {
    String name
}
```

the artifact is exactly (2-space pretty JSON, deterministic key order):

```json
{
  "formatVersion": 1,
  "surface": "mox",
  "sourceHash": "sha256:06537d40d3cb9b566d340813965e61d635c78fd9b37a427e19394303e9b1a4b4",
  "generator": {
    "name": "pherolang",
    "version": "0.3.0"
  },
  "degraded": false,
  "spans": [
    { "start": 0, "end": 7, "class": "keyword", "modifiers": [] },
    { "start": 8, "end": 12, "class": "namespace", "modifiers": ["declaration"] },
    { "start": 14, "end": 19, "class": "keyword", "modifiers": [] },
    { "start": 20, "end": 25, "class": "type", "modifiers": ["declaration"] },
    { "start": 26, "end": 27, "class": "punctuation", "modifiers": [] },
    { "start": 32, "end": 36, "class": "property", "modifiers": ["declaration"] },
    { "start": 37, "end": 43, "class": "type", "modifiers": ["defaultLibrary"] },
    { "start": 44, "end": 45, "class": "punctuation", "modifiers": [] }
  ]
}
```

The source above ends with a newline; its hash covers exactly those bytes.

| Field | Type | Meaning |
|---|---|---|
| `formatVersion` | number | Compatibility gate. v1 is `1`; readers reject an absent or non-1 value |
| `surface` | string | Registry id (`mox`, `ifml`, `ddd`, `actor`, `evt`, `deploy`, `uc`, `cmmn`, `dmn`) |
| `sourceHash` | string | `sha256:<lowercase hex>` over the exact source bytes that were highlighted |
| `generator` | object | `{ "name": "pherolang", "version": "<crate version>" }`; informational, not a compatibility gate |
| `degraded` | boolean | `true` when the semantic pass was unavailable or incomplete; the artifact is still successful |
| `spans` | array | Ordered, non-overlapping span objects: `start`, `end`, `class`, `modifiers` |

### Rules

- **Casing.** Struct fields and modifier strings are camelCase
  (`sourceHash`, `defaultLibrary`); class strings are lowercase
  (`keyword`).
- **No source text.** Only `sourceHash` is emitted; the consumer already has
  the source. No path, file name, timestamp, host, environment, or absolute
  path appears.
- **Deterministic.** Key order is the order above; span order is by
  `start`; `modifiers` is the canonical modifier order with no duplicates.
  Serialization is 2-space pretty JSON. Two runs are byte-identical.
- **Half-open bytes.** `start`/`end` are byte offsets into the source,
  `[start, end)`, sorted and non-overlapping. Gaps are implicit.
- **Forward compatibility.** Unknown fields are ignored on read. New
  optional fields are additive and `#[serde(default)]`-friendly; new
  classes/modifiers are additive strings, but a forward-compatible consumer
  must skip a span it cannot name while the reference Rust model rejects an
  unknown value instead of guessing.
- **Versioning.** Additive changes keep `formatVersion: 1`. A re-purpose of
  a field, a unit change, or a removed/renamed field bumps the version and
  is a breaking change.
- **Empty input.** An empty source emits an empty `spans` array and
  `"degraded": false`.

### Directory shape

Directory and multi-file input emit **one artifact per input file**, never a
single array artifact. The layout mirrors `pherolang build`:

- `<output>/<input path>.json` (JSON) or `<output>/<input path>.html`
  (HTML);
- `<input path>` is the path as given, with its directories preserved and
  root/prefix components dropped, so an absolute input never swallows the
  output directory;
- ordering is lexicographic by the expanded input path;
- the output directory is created as needed; nothing is written outside it.

`stdout` is **single-input only**. A single expanded input prints to stdout
unless `--output` names a file (written exactly there) or an existing
directory (the same mirrored path is used inside it). Multiple inputs
require `--output` to be a directory.

## HTML contract

The HTML renderer is a deterministic fragment for the site and editor
tooling, not a full document and not a theme:

```html
<pre class="pherolang-highlight"><code class="language-mox"><span class="hl-keyword">package</span> <span class="hl-namespace hl-declaration">shop</span></code></pre>
```

- Wrapper exactly: `<pre class="pherolang-highlight"><code
  class="language-<surface>">` … `</code></pre>`; no `<html>`/`<body>` and
  no added indentation.
- Each span is `<span class="hl-<class> hl-<kebab-modifier> …">` with the
  class first and modifiers in canonical order. Gaps are emitted as escaped
  source text.
- Escape the source exactly (`&`, `<`, `>`, and `"` in attributes); preserve
  newlines and whitespace. Never emit raw source.
- No inline styles, colours, theme variables, scripts, or data attributes.
  The renderer emits semantic class names only.
- The output is deterministic and diffable; one block per input.

The HTML format is a convenience over the JSON contract. If a consumer needs
the canonical data (hashes, degraded flag, exact classes), it reads JSON.

## The `pherolang highlight` command

```sh
pherolang highlight <file|dir|-> ... \
  [--format json|html] \
  [--mode strict|lenient] \
  [--surface <id>] \
  [--output <path>]
```

Examples:

```sh
pherolang highlight model.mox --format json
pherolang highlight model/ --format html --output target/highlight
printf 'class A { }' | pherolang highlight --surface mox --format json -
```

- Inputs are one or more files, a directory, or `-` for stdin. Directories
  expand exactly like `check`/`build` (supported surfaces only;
  formatter-golden `X.fmt.<ext>` files are skipped).
- `--format` defaults to `json`.
- `--mode` defaults to `strict` for files and directories and `lenient` for
  stdin and docs fences.
- `--surface` takes a registry id, an extension (with or without the dot),
  or an alias. It is required for stdin and may force one input file; it
  cannot be combined with a directory input or several files. Otherwise the
  surface is inferred from the extension.
- `--output` is a file for one input or a directory for multiple inputs;
  multiple inputs require a directory. See [Directory shape](#directory-shape).
- The command is offline: no network, and no filesystem access beyond the
  inputs and outputs.
- Exit codes:
  - `0` — success, including a `degraded` result;
  - `1` — an input could not be read, its extension does not name a
    supported surface, or the output could not be written;
  - `2` — usage error (missing or unknown `--surface`, `--surface` with a
    directory or several files, stdin combined with other inputs, multiple
    inputs without a directory output, conflicting flags, unknown option).

## CSS and themes

The highlighter emits semantics; the site owns presentation.

- Style the stable class names (`.hl-keyword`, `.hl-type`,
  `.hl-default-library`, …). They are part of the contract; renaming one is
  a breaking change.
- Colour, contrast, typography, and light/dark belong to the site's token
  layer and stylesheets. No colour is emitted or hardcoded by the
  highlighter.
- Theme selectors may compose class and modifier, but must not depend on
  source element names or inline styles.
- Contrast is measured in both light and dark modes by the site's
  accessibility gate, not by this crate.

## Quality gates

The highlighting slice is gated like the rest of the compiler:

- **Golden JSON** for every `examples/support/support.*` surface, under
  `tests/highlight/`.
- **Golden HTML** for a representative fixture per surface.
- **Docs-fence corpus**: every `mox`, `ifml`, `ddd`, `actor`, `evt`,
  `deploy`, `uc`, `cmmn`, and `dmn` fence in `docs/**` highlights without
  failure, and a representative fence is a golden.
- **Span invariants**: sorted, non-overlapping, in bounds, no empty spans,
  no split comments/strings.
- **Unicode/UTF-16**: emoji, combining characters, CRLF, and non-ASCII
  identifiers produce correct byte spans and UTF-16 conversion; the source
  hash covers the exact bytes.
- **Determinism**: a second run is byte-identical, in stdout and directory
  mode.
- **Performance budget**: one lex and one parse per input; the support
  corpus plus the docs-fence corpus completes within the coarse threshold
  recorded with the test (a CI budget, not a microbenchmark).
- **Drift tests**: every token variant is classified or explicitly ignored;
  every word-table entry is registered; every logos keyword is classified;
  all nine surfaces have a registry entry; contextual words do not leak
  between surfaces; `formatVersion` rejection and "no absolute paths" are
  pinned.
- **No hand-maintained site list**: a keyword added to a parser table must
  never require editing `site/` or `phero-highlight`.

Regenerate the goldens with:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test -p phero-cli --test cli highlight
```

## Out of scope

These are later issues in the epic and are **not** part of this slice:

| Issue | Scope |
|---|---|
| SH4 (#290) | Static site integration for both origins, themes/CSS tokens, offline generated data |
| SH5 (#291) | Generated TextMate/VS Code grammars and language configuration |
| SH6 (#292) | LSP semantic tokens, hover, definitions, and diagnostics |
| SH7 (#293) | Interactive browser hover/editor surface |
| SH8 (#294) | Release gates, accessibility/contrast measurements, and documentation beyond this slice |

Also out of scope: a second lexer or parser, a watch mode, publishing a
standalone schema package, and any client-side JavaScript on static pages.

## References

- Grammar authority: [`crates/phero-syntax/src/parser/mod.rs`](../crates/phero-syntax/src/parser/mod.rs)
  and [`crates/phero-ifml/src/parser/mod.rs`](../crates/phero-ifml/src/parser/mod.rs)
- Expression language: [EXPRESSIONS.md](EXPRESSIONS.md)
- Spelling contract: [NAMING.md](NAMING.md) — product "Phero Lang", binary
  `pherolang`, crates `phero-*`
- Artifact/wire conventions: [BACKENDS.md](BACKENDS.md) and the `phero-ir`
  crate docs
- Documentation index: [README.md](README.md)
