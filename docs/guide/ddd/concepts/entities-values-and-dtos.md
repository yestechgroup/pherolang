---
title: Entities, values, and DTOs
description: The three design shapes a domain class can take, what each one promises, and how to choose between them.
type: concept
language: ddd
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md]
related: [docs/guide/ddd/how-to/define-a-value-object.md, docs/guide/ddd/reference/designs.md]
lastReviewed: 2026-11-21
---

# Entities, values, and DTOs

An application design gives each domain class one of three meanings: it can be
an **entity** with identity and a lifecycle, a **value** defined entirely by
its data, or a **DTO** — a transport shape that crosses a boundary without
becoming part of the domain.

## Overview

The domain model declares structure. The design declares semantics: whether
instances are distinguished by identity, whether they may change, and whether
they are part of the domain at all. Those three answers are the three design
shapes.

| Shape | Identity | Mutability | Stored | Purpose |
|---|---|---|---|---|
| `entity` | yes | mutable | repository | A domain thing with a lifecycle. |
| `value` | no | strictly immutable | optional | A domain thing defined by its data. |
| `dto` | no | data only | never | A shape crossing an application boundary. |

The choice is per application, not per class. The same `.mox` class may be an
entity in one application and a value in another; each design states its own
answer.

## Why does it matter?

Equality, persistence, and the API surface all follow from this choice. If a
recorded decision is designed as an entity, two decisions with identical data
are still different objects, and the application has to say what distinguishes
them. If a mutable object is designed as a value, structural equality becomes
unstable. If a transport shape is designed as an entity, the boundary leaks
domain identity and lifecycle into callers who should not see either.

Making the choice explicit in one place is what lets every consumer — storage,
services, the interaction layer — agree on it.

## How it works

**Entity.** The class gains identity and a lifecycle. An entity may carry a
`repository` (the built-in operations plus declared signatures), a `machine`
over an enum-typed feature, invariants, and the aggregate flags. A root can be
authored with `root`, or derived from containment in the domain model.

**Value.** The class is strictly immutable, with no identity, deep
immutability, and structural equality over its stored features. There is no
opt-out: a class that must change is an entity, and a value that needs to name
an entity refers to it by id.

**DTO.** The class is a transport shape, not a domain concept. A mirror takes
every stored feature of the class; a projection selects, renames, or derives
members; a standalone DTO declares them. Direction is derived: any derived
member makes the shape an output, and an output shape cannot be a parameter.

## Example

All three shapes in one module:

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        entity Ticket repository TicketRepository {
            findById;
            findAll;
            save;
            delete;
        }
        machine state: TicketState {
            initial Open;
            Open -> InProgress via Claim;
            InProgress -> Resolved via Resolve;
            terminal Resolved;
        }

        value RefundDecision nonPersistent

        dto NewTicket {
            String title;
            boolean internal;
        }
    }
}
```

`Ticket` is an entity because it has identity and moves through a lifecycle.
`RefundDecision` is a value because it records facts and never changes.
`NewTicket` is a DTO because it is the shape a caller sends, not a thing the
domain stores.

## Relationship to other concepts

- The **domain model** ([the .mox overview](../../mox/overview.md)) declares
  the classes, features, and enums these designs refer to.
- A **repository** belongs to an entity design; a value or a DTO cannot carry
  one.
- A **state machine** belongs to an entity design; a value is immutable and a
  DTO has no lifecycle.
- An **aggregate** is a group of entity designs with one root; membership and
  derivation are covered in [Designs](../reference/designs.md).
- A **service** uses these shapes in its operation signatures; a DTO parameter
  is how a boundary stays narrow.

## Further reading

- [Define a value object](../how-to/define-a-value-object.md) — the strict
  value rules in practice.
- [Designs](../reference/designs.md) — every declaration, flag, and diagnostic.
- [Relationship with Sculptor](relationship-with-sculptor.md) — where these
  shapes come from.
