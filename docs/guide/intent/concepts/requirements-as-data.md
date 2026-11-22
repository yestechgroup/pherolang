---
title: Requirements as data
description: Why a requirement becomes a declaration with a stable id, a quoted statement, and links, so the project can check what expresses and verifies it.
type: concept
language: intent
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/intent/overview.md]
related: [docs/guide/intent/reference/intent-specifications.md]
lastReviewed: 2026-11-21
---

# Requirements as data

A requirement becomes data when it has three parts a tool can read: an id that
identifies it, a statement that says it, and links that point at the parts of
the model that express and verify it.

## Overview

The three parts answer three different questions. The id answers "which
requirement is this?", across files, runs, and tools. The statement answers "what
does it say?", in the author's own words. The links answer "where does it live?",
by naming model elements rather than copying them.

None of the three is prose that happens to sit near the model. Each is a
declaration, so the project can check it: the id against a fixed shape, the
statement against being non-empty, the links against the elements the
application actually declares. An intent that nothing expresses, or nothing
verifies, is reported as a gap.

## Why does it matter?

Requirements written beside a model have no mechanical connection to it. The
document says a refund above a limit needs a human; the code has an approval
step; nothing proves the two are the same claim, and nothing notices when one
changes. The usual repair is a traceability spreadsheet, maintained by hand and
stale by the next release.

Once a requirement is a declaration, the connection is checked. A renamed
operation breaks a link and the project says so. A requirement that no element
expresses is a warning, not a silent omission. And the coverage question — "does
every requirement have both something that expresses it and something that
verifies it?" — is answered by the same command that checks the rest of the
project.

The cost of ignoring this is not only stale documentation. It is being unable to
answer, with evidence, which part of the system honors a requirement, and which
check would catch it if that part stopped working.

## How it works

- **The id** is written verbatim in a fixed shape: an uppercase lead segment
  plus at least one hyphen-separated all-digits segment, as in `REFUND-001`. The
  text is kept exactly as typed, so `REFUND-001` and `REFUND-1` are different
  ids. Declaration order is irrelevant; moving an intent to another line does
  not change its identity.
- **The statement** is a quoted string holding the requirement in words. It is
  kept as written apart from surrounding whitespace, and never interpreted or
  evaluated. An empty statement is refused, because an id with no readable
  claim is not a requirement.
- **The links** fill two slots. `element` names what expresses the requirement:
  a `class`, `capability`, `event`, `view`, `command`, or `op`. `check` names
  what verifies it: an `invariant` (a class in the application design that
  carries invariants) or a `decision`.
- **The kind is checked first.** When the file is read, each kind word is
  checked against its slot's closed list. A kind in the wrong slot, or an
  invented kind, is an error at that point.
- **The names are checked later, against the project.** The file only carries
  the names as authored. When the whole application is checked, the coverage
  pass resolves them: a dotted `package.Class` or `Service.operation` first,
  then a bare name that must match exactly one target. Resolution is never part
  of reading one file.
- **Coverage is reported, not hidden.** Every intent is expected to resolve at
  least one element and at least one check. A missing slot produces a warning;
  a name that resolves to nothing, or to several things, produces an error. A
  file whose intents have no links at all is still valid — it has simply made no
  claim about where the requirement lives, and the project reports that.
- **Only the domain is imported.** An `import` names a `.mox` domain model and
  exists so the file is grouped with the right application. It never brings a
  link target into scope: links are names across the application, not imports.

## Example

One requirement with one element link and one check link:

```intent
intent REFUND-001 "A refund above the desk's auto-approval limit is decided by a human." {
    element capability:ApproveRefund;
    check decision:RefundAuthority;
}
```

The file reads on its own, but its two names are resolved only when the project
is checked: `ApproveRefund` must be a capability of the application's actor
policy, and `RefundAuthority` must be a decision of one of its decision models.
A real file also imports the domain model it belongs to, which is what places it
in that application; the [overview](../overview.md) shows the import in place.

## Relationship to other concepts

- **The domain model** ([.mox](../../mox/overview.md)) supplies the `class` links.
  The import that groups an intent file with an application names this model.
- **Actor policies** ([.actor](../../../LANGUAGE.md)) supply `capability` links: the
  capabilities granted to actors are what a requirement can name as its
  enforcer.
- **Event contracts** ([.evt](../../../EVENTS.md)) supply `event` links.
- **Application designs** ([.ddd](../../ddd/overview.md)) supply `op` links, naming
  a service operation, and `invariant` links, naming a class that carries
  invariants.
- **Decision models** ([.dmn](../../dmn/overview.md)) supply `decision` links: the
  rules that verify a requirement.
- **Interaction flows** ([.ifml](../../ifml/overview.md)) supply `view` and
  `command` links.
- **The coverage pass** is what turns names into evidence; it is described in
  the [engineering reference](../../../INTENTS.md).
- **The semantic graph** ([the graph reference](../../../GRAPH.md)) carries one
  node per intent and an `expresses` or `verifiedBy` edge per resolved link.

## Further reading

- [Intent specifications](../reference/intent-specifications.md) — the syntax,
  the link kinds, and every report code.
- [Intent specifications (.intent)](../overview.md) — the language in one page.
- [The intent specification reference](../../../INTENTS.md) — the file format
  and the coverage pass in engineering detail.
