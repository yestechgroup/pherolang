---
title: Intent specifications (.intent)
description: State requirements as checked model data — stable ids, quoted statements, and links to the elements that express them and the checks that verify them.
type: overview
language: intent
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/intent/concepts/requirements-as-data.md, docs/guide/intent/reference/intent-specifications.md]
lastReviewed: 2026-11-21
---

# Intent specifications (.intent)

A `.intent` file states what a system must do and ties each statement to the
parts of the model that carry it out. Every requirement gets an id that does not
change, the requirement itself as a quoted sentence, and links to the elements
that express it and the checks that verify it.

## What is it?

An intent specification is a list of requirements written as declarations. One
`intent` declaration is one requirement: a stable id such as `REFUND-001`, a
quoted statement such as "A refund above the desk's auto-approval limit is
decided by a human.", and links that name what carries the requirement in the
rest of the project.

The links are names, not copies. `element class:Ticket` says a class expresses
the requirement; `element capability:ApproveRefund` says the capability that
enforces it; `check decision:RefundAuthority` says the decision that verifies
it. The names are resolved against the whole application whenever the project is
checked, so a requirement and its evidence cannot drift apart silently: either
the names resolve, or the project reports them.

## What problems does it solve?

Requirements usually live beside the model rather than in it: a document, a
ticket, a spreadsheet of traceability links. Nothing checks that the document
still matches the system, so "which operation serves this requirement?" and "is
this requirement verified at all?" are answered by hand, late, and differently
each time.

Writing requirements as declarations makes those questions cheap. The claim and
the evidence sit in one place, the project reports a requirement that nothing
expresses or nothing verifies, and a rename in any other language shows up as a
broken link instead of a stale paragraph. The rule is honesty over reach: an
uncovered requirement is reported, never quietly dropped.

## Key capabilities

- **Stable ids.** An uppercase lead segment plus numbered segments
  (`REFUND-001`, `REFUND-DESK-007`). The id is kept exactly as written, so
  leading zeros survive and identities do not shift when declarations move.
- **The requirement in words.** A quoted statement, kept as written apart from
  surrounding whitespace and refused if empty, so a reader can understand the
  claim without opening the rest of the model.
- **Element links.** `class`, `capability`, `event`, `view`, `command`, and
  `op` say what expresses the requirement.
- **Check links.** `invariant` and `decision` say what verifies it.
- **Project-wide resolution.** The kind of each link is checked when the file is
  read; the names are resolved against the whole application.
- **Coverage reporting.** A requirement with no resolved element or no resolved
  check is reported; a name that resolves to nothing, or to several things, is
  an error.
- **Traceability in the semantic graph.** Each intent becomes a node, joined to
  what it names by `expresses` and `verifiedBy` edges.

## A simple example

One requirement, linked to a class, a capability, and a decision:

```intent,fixture=examples/support
import "support.mox";

intent REFUND-001 "A refund above the desk's auto-approval limit is decided by a human." {
    element class:Ticket;
    element capability:ApproveRefund;
    check decision:RefundAuthority;
}
```

The import names the domain model the file belongs to, which is how the project
groups the file with the rest of the support application. The three links reach
into three different languages — a class from the domain model, a capability
from the actor policy, and a decision from the decision model — and checking the
project confirms all three.

## How it fits into Phero Lang

`.intent` is the requirements dimension. It imports one domain model and links
into every other dimension:

| Layer | What an intent links to |
|---|---|
| [Domain models](../mox/overview.md) (`.mox`) | `class` — the classes that express the requirement. |
| [Actor policies](../../LANGUAGE.md) (`.actor`) | `capability` — the capabilities granted to actors. |
| [Event contracts](../../EVENTS.md) (`.evt`) | `event` — what the application publishes. |
| [Application designs](../ddd/overview.md) (`.ddd`) | `op` — a service operation that carries the requirement; `invariant` — a class whose invariants verify it. |
| [Decision models](../dmn/overview.md) (`.dmn`) | `decision` — the rules that verify the requirement. |
| [Interaction flows](../ifml/overview.md) (`.ifml`) | `view` and `command` — what people see and run. |

Only the domain model is imported, and only so the file is grouped with the
right application. Every other link is a name resolved across the application,
not an import: one requirement can join a class, a capability, a decision, and a
screen without importing four files. The project check then reports what is
missing instead of assuming the requirement is covered.

## Getting started

There is no tutorial for `.intent` yet. Start with:

- [Requirements as data](concepts/requirements-as-data.md) explains why a
  requirement is written as a declaration instead of prose beside the model.
- [Intent specifications](reference/intent-specifications.md) states the syntax,
  the link kinds, and the coverage rules.
- The [support intent file](../../../examples/support/support.intent) is a
  complete worked example the project's tests compile on every change.

## Further reading

- [Requirements as data](concepts/requirements-as-data.md) — the concept behind
  the three parts of a requirement.
- [Intent specifications](reference/intent-specifications.md) — the reference
  for ids, statements, links, and report codes.
- [The intent specification reference](../../INTENTS.md) is the engineering
  reference for the file format and the coverage pass.
- [The semantic graph](../../GRAPH.md) describes the nodes and edges an intent
  contributes to the project-wide picture.
