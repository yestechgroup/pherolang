# Editors and the language server

`pherolang lsp` serves a language server over **stdin/stdout** — the standard
Language Server Protocol transport. It takes no flags and listens on no
port: your editor starts the process, speaks LSP to it over pipes, and owns
its lifetime. There is **no editor extension in this repository**; the
binary is the extension. Any editor that can drive a stdio language server
gets the same server.

## What the server provides

Verified against `pherolang 0.4.0` by driving the `initialize` handshake by
hand — the server advertises:

| Capability | Notes |
|---|---|
| hover | types and documentation from the compiler's own resolution |
| go-to-definition / find-references | resolution is the graph index's; an unbindable reference yields no location, never an error |
| rename | declaration-owned and **document-scoped** — a cross-document rename is refused, naming the owning file |
| completions | triggered on `.` |
| formatting | the per-surface formatter, preserving comments |
| document symbols | the file's outline |
| semantic tokens | the compiler-backed highlighting taxonomy (`docs/HIGHLIGHTING.md`) |
| code actions (quickfix) | where the compiler knows the fix |
| text sync | full document sync |

The scoping contract — one graph snapshot per directory, how surfaces
resolve, and what the editor shares with `pherolang graph build` — is owned
by [GRAPH.md](GRAPH.md#the-lsp-contract); this page deliberately does not
restate it. All nine surfaces (`.mox`, `.actor`, `.ddd`, `.evt`, `.dmn`,
`.deploy`, `.uc`, `.cmmn`, `.ifml`) are served by the one process.

A ten-second smoke test without an editor: start `pherolang lsp` in a
terminal, paste a framed `initialize` request, and read the capabilities
response; `shutdown` + `exit` terminates the process with code 0.

## Neovim

> **Untested.** The project has not executed this setup (no Neovim in the
> development environment). It uses only documented Neovim APIs
> (`vim.filetype.add`, `vim.lsp.start`) and is the standard shape for a
> stdio server — but verify against your Neovim version (≥ 0.8 for
> `vim.lsp.start`).

Neovim does not know the Phero Lang file extensions, so teach it the
filetypes first, then start the server per buffer:

```lua
-- ~/.config/nvim/lua/pherolang.lua, required from init.lua
vim.filetype.add({
  extension = {
    mox = 'mox', actor = 'actor', ddd = 'ddd', evt = 'evt',
    dmn = 'dmn', deploy = 'deploy', uc = 'uc', cmmn = 'cmmn', ifml = 'ifml',
  },
})

vim.api.nvim_create_autocmd('FileType', {
  pattern = { 'mox', 'actor', 'ddd', 'evt', 'dmn', 'deploy', 'uc', 'cmmn', 'ifml' },
  callback = function(args)
    vim.lsp.start({
      name = 'pherolang',
      cmd = { 'pherolang', 'lsp' },
      root_dir = vim.fs.root(args.file, { '.git' }),
    })
  end,
})
```

## VS Code

> **Untested.** VS Code has no built-in way to drive a raw stdio language
> server; it needs a generic LSP-client extension from the marketplace, and
> this repository ships none. The settings below show the *shape* such
> extensions consume — the exact keys belong to whichever extension you
> install, so treat this as a starting point, not a verified configuration.

```jsonc
// settings.json — shape varies by generic LSP-client extension
{
  "<extension>.servers": {
    "pherolang": {
      "command": "/usr/local/bin/pherolang",
      "args": ["lsp"],
      "languages": ["mox", "actor", "ddd", "evt", "dmn", "deploy", "uc", "cmmn", "ifml"]
    }
  }
}
```

Whatever the extension, the server invocation is the same one command:
`pherolang lsp`.

## Zed

> **Untested.** Zed runs custom language servers natively, but a server is
> only *attached to a file type* through a language definition, and Phero
> Lang is not one of Zed's built-in languages — so fully wiring it wants a
> small Zed extension that declares the languages. The server half alone is
> configured in user settings:

```json
// ~/.config/zed/settings.json
{
  "lsp": {
    "pherolang": {
      "binary": { "path": "/usr/local/bin/pherolang", "arguments": ["lsp"] }
    }
  }
}
```

Until a language extension maps the nine extensions to the `pherolang`
server, Zed will start it for nothing; the extension is the remaining work,
and this repository does not ship one.
