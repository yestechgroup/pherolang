---
title: Domain models (.mox)
description: Describe the structure of a problem domain — its classes, relationships, and rules — in a form Phero Lang can check and every other language builds on.
type: overview
language: mox
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/mox/concepts/modelling-not-coding.md, docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Domain models (.mox)

A `.mox` file describes what a system *is made of*: the things in its problem
domain, the data each thing carries, how the things relate, and which rules
always hold. It is the first file you write in a Phero Lang project, and every
other language in the ecosystem builds on the model it produces.

## What is it?

A domain model is a declaration of structure. You name the classes your domain
has — `Ticket`, `Customer`, `RefundRequest` — and describe each one with
features: stored values, relationships to other classes, computed values, and
operations. Phero Lang reads the file, checks that it is consistent, and
produces a structured model that tools can consume: generated code, JSON
Schemas, sample data, and the input to every other Phero language.

The language is declarative. You do not write storage code, constructors, or
serializers; those follow from the model. You say a ticket *has* a refund
amount that is never negative, and the rule follows the model wherever it goes.

## What problems does it solve?

Most systems describe their data three or four times: once in the database,
once in application classes, once in an API schema, and once in the tests. The
descriptions drift, and the drift is discovered in production.

A domain model removes the duplication: one description, checked once, from
which the other forms are produced. It also makes the domain's rules explicit.
"An order must have at least one line" stops being a comment on a repository
operation and becomes part of the model.

## Key capabilities

- **Classes with identity.** Mark a feature `id` when instances are
  distinguished by identity rather than by their values.
- **Relationships.** Model ownership with `contains`/`container` and links
  between independent objects with `refers`.
- **Enums and closed sets.** Represent a state machine's states or a
  fixed vocabulary as an enum whose literals carry stable integer values.
- **Value rules.** Declare bounds and patterns — `int stock { minimum 0 }`,
  `String sku { pattern "[A-Z]{3}-[0-9]{4}" }` — next to the feature they
  constrain.
- **Derived values and operations.** State what can be computed
  (`derived boolean refundable`) and what can be attempted
  (`op boolean raiseRefund(int cents)`).
- **Packages and reuse.** Split a large domain across files, import external
  JSON Schema types, and refer across packages.

## A simple example

A support ticket with a lifecycle, a stored amount, a computed fact, and one
operation:

```mox
package nz.example.support

enum TicketState {
    Open as "open" = 0
    InProgress as "in_progress" = 1
    Resolved as "resolved" = 2
}

class Ticket {
    id readonly String ticketNo
    String title
    int refundCents = 0
    TicketState state = Open
    derived boolean refundable { expr { refundCents > 0 } }
    op boolean raiseRefund(int cents) { expr { refundCents + cents <= 5000 } }
}
```

`Ticket` is identified by `ticketNo`, carries a state from a closed set, and
computes `refundable` from stored data. `raiseRefund` states the rule that one
refund may not exceed 5000 cents.

## How it fits into Phero Lang

`.mox` is the foundation. Each other language adds one dimension on top of the
same model:

| Language | Adds |
|---|---|
| `.ddd` | The application's architecture: modules, services, repositories, entity lifecycles. |
| `.evt` | Event contracts: what is published, on which channels, to which subscribers. |
| `.dmn` | Decision rules: how inputs map to outcomes. |
| `.cmmn` | Adaptive cases: work that cannot be fully planned in advance. |
| `.ifml` | Interaction flows: the screens users see and the paths between them. |
| `.uc` | Use case patterns over the whole system. |
| `.deploy` | Where the application runs and under which policies. |
| `.intent` | Requirements with stable ids, linked to model elements. |

A lending application, for example, might use `.mox` for customers and loans,
`.ddd` for the application architecture, `.dmn` for credit decisions, and
`.ifml` for the advisor's screens — one domain model underneath all of them.

## Getting started

- [Tutorial — your first domain model](../tutorials/your-first-domain-model.md)
  builds a small refund desk from an empty directory.
- [Add an enum to a domain model](../how-to/add-an-enum.md) is the one-task
  version of the enum you saw above.
- [Model a relationship](../how-to/model-a-relationship.md) covers ownership,
  links, and opposites.
- [Choose a collection](../how-to/choose-a-collection.md) covers lists, unique
  lists, and sets.

## Further reading

- [Modelling, not coding](../concepts/modelling-not-coding.md) explains why the
  model comes first.
- [Classes and features](../reference/classes.md) is the reference for
  everything a class can declare.
- [Enums, datatypes, and vocabularies](../reference/enums-datatypes-and-vocabularies.md)
  covers the rest of the type universe.
- [Packages and imports](../reference/packages-and-imports.md) covers
  multi-file models and imported types.
- [The .mox language reference](../../LANGUAGE.md) is the complete
  engineering reference, including grammar and tooling.
- The [support-desk example](../../../examples/support/support.mox) is a
  complete domain model the project's tests compile on every change.
