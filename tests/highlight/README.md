# Highlight goldens

Byte-pinned output for `pherolang highlight`.

- `json/support.<surface>.json` — one artifact for every
  `examples/support/support.*` source, generated in `--mode strict`.
- `json/fence.mox.json` — a docs-fence-shaped stdin input.
- `html/<surface>.html` — one small snippet per surface from `snippets/`.
- `snippets/<surface>.<extension>` — the HTML inputs. They are deliberately
  outside `tests/conformance/` so `pherolang check tests/conformance/` never
  treats a snippet as a standalone model.

Regenerate after a deliberate engine or fixture change, then read the diff —
the goldens are the contract:

```sh
PHERO_UPDATE_FIXTURES=1 cargo test -p phero-cli --test cli highlight
```
