# Highlight goldens and corpus

Byte-pinned output and checked inputs for `pherolang highlight`.

- `json/support.<surface>.json` — one artifact for every
  `examples/support/support.*` source, generated in `--mode strict`.
- `json/fence.mox.json` — a docs-fence-shaped stdin input.
- `json/corpus.<case>.json` — exact-span artifacts for the representative
  corpus cases (one per surface family plus every shared byte case).
- `html/<surface>.html` — one small snippet per surface from `snippets/`.
- `snippets/<surface>.<extension>` — the HTML inputs. They are deliberately
  outside `tests/conformance/` so `pherolang check tests/conformance/` never
  treats a snippet as a standalone model.
- `invalid/format-version-{2,absent}.json` — committed fixtures the reference
  model must reject (`UnsupportedFormatVersion`), so the compatibility gate is
  exercised on bytes, not only on an inline string.
- `corpus/` — the checked corpus. One directory per surface (`valid`,
  `truncated`, `parse-error`, plus the surface-specific cases) and `shared/`
  for Unicode/emoji/combining, CRLF, escapes, and doc comments. The CRLF case
  is kept byte-exact by `/.gitattributes`.
- `summary/corpus.json` — a compact per-case class/modifier histogram.
- `summary/docs-fences.json` — one compact row per Phero Lang fence in
  `docs/**/*.md`, so every documentation fence is covered and a changed fence
  is visible in review.

Regenerate after a deliberate engine, parser, vocabulary, or docs change, then
read the diff — the goldens are the contract:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test -p phero-cli --test cli highlight
PHERO_UPDATE_FIXTURES=1 cargo test -p phero-highlight --test corpus
```

Nothing is auto-blessed in CI.
