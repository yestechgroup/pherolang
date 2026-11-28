# Architecture decision records

Numbered, immutable records of decisions that are expensive to reverse. One
file per decision, `NNNN-kebab-case-title.md`; the status line is the index.

| ADR | Decision | Status | Date |
|---|---|---|---|
| [0001](0001-rebrand-to-phero-lang.md) | Rebrand rexlang to Phero Lang | Accepted | 2026-11-14 |
| [0002](0002-static-hover-reference-panels.md) | Static hover reference panels, not a browser IDE | Accepted | 2026-11-16 |
| [0003](0003-compiler-backed-highlighting-is-the-source-of-truth.md) | Compiler-backed highlighting is the source of truth | Accepted | 2026-11-17 |
| [0004](0004-supported-platforms-linux-only.md) | Support Linux x86_64 only; drop Windows, macOS, and aarch64 | Accepted | 2026-11-16 |
| [0005](0005-shared-surface-kernel-layering.md) | The surface kernel stays shared; the model stays typed per surface (SK1 layering audit) | Accepted | 2026-11-17 |
| [0010](0010-intent-specifications-are-a-declaration-surface.md) | Intent specifications are a declaration surface | Accepted | 2026-11-20 |
| [0011](0011-documentation-quality-standard.md) | Documentation quality is a normative, hard-enforced standard | Accepted | 2026-11-21 |
| [0012](0012-rune-is-an-in-tree-surface.md) | Rune (`.rosetta`) is an in-tree surface; sigil comes into the monorepo | Accepted | 2026-11-28 |
| [0013](0013-rune-keeps-its-own-token-enum.md) | Rune keeps its own token enum inside phero-syntax (amends 0005 D3 for Rune) | Accepted | 2026-11-28 |
