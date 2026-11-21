---
title: Modelling, not coding
description: Why a domain model is written before and separately from the code that consumes it, and what that separation buys.
type: concept
language: mox
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/mox/overview.md, docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Modelling, not coding

A Phero Lang domain model describes a problem domain — what things exist, how
they relate, which rules hold — and nothing else. It is not a sketch that a
program then replaces. It is the description the program is built from.

## Overview

Most software describes the same domain in several places: the database schema,
the application's classes, the API contract, the validation in tests, and the
prose in a design document. Each description is written for a different tool,
and each is maintained separately. The cost is not the writing; it is the
drift, and the debugging that follows when two descriptions disagree.

A domain model is one description, written for the domain rather than for a
tool. Phero Lang checks it and produces the other forms from it: code that
holds the data, JSON Schemas that describe it, and the starting point for every
other Phero language. The model is data, so it can be reviewed, compared, and
generated from — properties that hand-written classes do not have.

## Why does it matter?

- **One source of truth.** A feature is declared once. If it changes, the change
  has a single reviewable diff.
- **Rules live with the data.** "Refunds never exceed 5000 cents" belongs next
  to `refundCents`, not inside one operation of one service.
- **Drift becomes impossible in the parts that matter.** The schema and the
  application cannot disagree about a feature's type, because they are produced
  from the same declaration.
- **The domain is reviewable by the people who own it.** A domain expert can
  read `contains OrderLine[] lines` without knowing a programming language.
- **Tools compose.** Because every language in the ecosystem reads the same
  model, decisions, cases, and interaction flows all refer to the same
  `Customer`.

## How it works

The model is made of declarations:

- A **class** names a kind of thing and lists its **features**.
- A feature either stores a value (an attribute), owns other objects
  (`contains`), links to an independent object (`refers`), computes a value
  (`derived`), or declares an operation with a rule (`op`).
- Identity is explicit: a feature marked `id` tells the model that two objects
  with the same data are still different things.
- An **enum** closes a set of values; a **datatype** wraps a platform type
  without exposing it.

Phero Lang then checks the model. It resolves every name, verifies that
opposites pair up, that constraints fit the type they constrain, and that
declarations do not collide. When it accepts the model, you have something
stronger than a file that parses: you have a domain that has been checked for
internal consistency.

## Example

Ownership and identity, side by side. An order owns its lines; the lines point
back to the order:

```mox
package nz.example.orders

class Order {
    id readonly String orderNo
    contains OrderLine[] lines opposite order
}

class OrderLine {
    String sku
    int quantity { minimum 1 }
    container Order order opposite lines
}
```

`contains` says the lines are part of the order and are stored with it. The
`container` back-pointer is the child's side of the same relationship. Neither
side can be declared alone with an `opposite`; the model checks that they
agree.

## Relationship to other concepts

- A **domain model** describes structure; an **application design** (`.ddd`)
  describes the application's modules, services, and entity lifecycles on top
  of it. See [the domain models overview](../overview.md).
- **Events** (`.evt`) describe what the application publishes; they are typed
  against the same model.
- **Decisions** (`.dmn`) describe rules that map inputs to outcomes, and
  **cases** (`.cmmn`) describe work that cannot be fully planned.
- **Interaction flows** (`.ifml`) describe what users see — again over the same
  model.

The model is the stable center; each other language adds one dimension and
imports the model rather than restating it.

## Further reading

- [Classes and features](../reference/classes.md) — the exact forms for every
  declaration used here.
- [Tutorial — your first domain model](../tutorials/your-first-domain-model.md)
  — build one from an empty directory.
- [The .mox language reference](../../../LANGUAGE.md) — the complete
  engineering reference for the language.
