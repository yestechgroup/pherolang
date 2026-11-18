# Installing Phero Lang

`pherolang` ships as a single native binary and reads nothing from the network
while it compiles. This page is the **one normative install guide**: every
other page links here instead of restating an install step. The truth it
states is the release's own —
[ADR 0004](adr/0004-supported-platforms-linux-only.md):

> **The supported platform is `x86_64-unknown-linux-gnu` (Linux on 64-bit
> Intel/AMD, glibc).** aarch64, macOS, and Windows are unsupported and
> untested: the release publishes no asset for them, and no job executes one.
> `cargo install` (below) builds from source and may work on other platforms,
> but that is not a support claim.

Everything below was executed against the
[v0.4.0 release](https://github.com/yestechgroup/pherolang/releases/tag/v0.4.0).
Substitute the tag you want in the URLs and archive names; the shapes do not
change between releases.

## Install from a release

Download the archive and its checksum manifest, verify, then move the binary
onto your `PATH`:

```sh
cd "$(mktemp -d)"
curl -LO https://github.com/yestechgroup/pherolang/releases/download/v0.4.0/pherolang-0.4.0-x86_64-unknown-linux-gnu.tar.gz
curl -LO https://github.com/yestechgroup/pherolang/releases/download/v0.4.0/SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
tar -xzf pherolang-0.4.0-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 pherolang /usr/local/bin/pherolang
```

`sha256sum -c` prints one line per archive it finds —
`pherolang-0.4.0-x86_64-unknown-linux-gnu.tar.gz: OK` — and exits `1`, naming
the file, if a single byte differs.

`/usr/local/bin` is on most `PATH`s already. To install without `sudo`
instead, put the binary in your home directory and make sure that directory
is on `PATH`:

```sh
install -m 0755 pherolang "$HOME/.local/bin/pherolang"
export PATH="$HOME/.local/bin:$PATH"   # if ~/.local/bin is not on your PATH yet
```

The archive contains a single file — the binary, at the archive root. There
is no installer, no service, and no configuration file.

## Verify the install

`--version` prints the version **and the compile target** — that pair is the
provenance you quote in a bug report:

```console
$ pherolang --version
pherolang 0.4.0 (x86_64-unknown-linux-gnu)
```

A second smoke: the diagnostic registry lists its bands without touching a
model file.

```console
$ pherolang explain --list
pherolang diagnostic codes — `pherolang explain <code>` prints one entry

BAND              RANGE            SEVERITY  NEXT FREE
ifml.resolver     E0001–E0099      error     E0010
...
```

With a model file handy, `pherolang check <file>` compiles it and prints
diagnostics grouped per file; the
[getting-started walkthrough on the site](https://pherolang.com/getting-started)
compiles the canonical `examples/support.mox`.

## Upgrade

Re-run the release install for the new tag: download, verify, and `install`
overwrites the old binary in place. There is nothing else to migrate —
`pherolang` keeps no state between runs.

## Uninstall

Delete the binary from wherever you installed it:

```sh
sudo rm /usr/local/bin/pherolang      # release install, system location
rm "$HOME/.local/bin/pherolang"       # release install, home location
cargo uninstall phero-cli             # cargo install (below)
```

## Install from source with cargo

If you have a Rust toolchain (via [rustup](https://rustup.rs)), cargo can
build and install the compiler straight from the release tag:

```sh
cargo install --git https://github.com/yestechgroup/pherolang --tag v0.4.0 --locked phero-cli
```

This compiles the workspace's release profile — minutes, not seconds — and
puts `pherolang` on `~/.cargo/bin` (usually already on `PATH`). `--locked`
builds the release's own `Cargo.lock`. There is no checksum step in this
path: the artifact is produced on your machine, not downloaded. Upgrading is
the same command with the new tag; uninstalling is `cargo uninstall
phero-cli`.

Crates are deliberately **not** published to crates.io before 1.0 —
[VERSIONING.md](VERSIONING.md) owns that decision; `cargo install --git` is
the supported source path.

## Editors

`pherolang lsp` serves a language server over stdio — hover, go-to-definition,
rename, completions, semantic tokens, formatting. Editor setups (Neovim, VS
Code, Zed) live in [EDITORS.md](EDITORS.md).
