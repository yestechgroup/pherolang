# Security policy

## Supported versions

Phero Lang is pre-1.0: only the latest 0.x minor line receives fixes. Older
minors are unsupported; `pherolang --version` names the exact build.

## Reporting a vulnerability

Report privately — do not open a public issue for a suspected vulnerability.
The primary channel is a GitHub private security advisory:
<https://github.com/yestechgroup/pherolang/security/advisories/new>

If you cannot use GitHub, write to `security@pherolang.com` instead. Use one
channel, not both, and send exploit details nowhere else.

A report we can act on contains:

- a minimal reproducer — the smallest source tree, artifact, or command
  sequence that shows the problem;
- the affected version and the full output of `pherolang --version`;
- what you expected and what happened, including any diagnostic output;
- your reading of the impact, if you have one.

## What to expect

These are expectations we aim to meet, not a service-level agreement.

- We aim to acknowledge a report within five working days, and to say then
  whether we could reproduce it.
- We will keep you informed as we investigate. If the report is out of
  scope, we will say so and explain why.
- Disclosure is coordinated: we agree a publication date with you, and we
  do not publish an advisory before a fix or a mitigation is available.
- We credit the reporter in the advisory unless you ask us not to.

## Scope

In scope:

- the compiler and the `pherolang` binary — a hostile source tree that drives
  the parser, resolver, type checker, or IR lowering into a panic, unbounded
  resource use, or a write outside the output directory;
- the language server (`phero-lsp`) — the same classes of failure reached
  through the LSP transport, plus anything that makes the server act beyond
  the documents it was given;
- the generated-artifact wire format — a malformed or hostile artifact that
  a consumer accepts although the compiler would never emit it, and any
  bypass of the `formatVersion` gate.

Out of scope:

- code generated into your project, and generated output you have since
  edited — the artifact is the boundary;
- the external toolchains Phero Lang interoperates with — sigil,
  restingstate/xstate-ir, dsntk, and cmmn-rs — which have their own security
  channels; report to them directly.

Ordinary bugs and feature requests belong in the issue tracker, not here.
