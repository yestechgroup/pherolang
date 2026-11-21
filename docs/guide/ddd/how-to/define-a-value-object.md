---
title: Define a value object
description: Design a class as a strictly immutable value — no identity, no mutable state, structural equality — and know which mistakes the compiler refuses.
type: how-to
language: ddd
section: how-to
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/ddd/tutorials/your-first-application-design.md]
related: [docs/guide/ddd/concepts/entities-values-and-dtos.md, docs/guide/ddd/reference/designs.md]
lastReviewed: 2026-11-21
---

# Define a value object

Use this page when a class is defined entirely by its data: two instances with
the same values are interchangeable, and nothing about the instance changes
after it is created.

## Problem

A recorded decision, a measurement, or a money amount should not be mutable.
If it can change, equality becomes ambiguous, and "the same decision" stops
having a stable meaning. You need the design to state that the class is a
value, and you need the compiler to refuse the spellings that would break it.

## Prerequisites

- The domain class already exists in a `.mox` model.
- Every stored feature of that class is declared `readonly`; the class carries
  no `id` feature.
- The [overview](../overview.md) read once.

## Solution

**1. Design the class as a value.**

```ddd,fragment
value RefundDecision
```

**2. Add `nonPersistent` when the value never reaches storage.**

```ddd,fragment
value RefundDecision nonPersistent
```

**3. Make the domain class satisfy the four properties.** A value has no
identity, is immutable, is deep-immutable (nested values and collections
included), and compares structurally over its stored features. In the domain
model, that means `readonly` on every stored feature, no `id`, no `contains`
or `refers` toward a designed entity, and no `abstract value`.

**4. Check the design.**

```sh
pherolang check design.ddd
```

The compiler checks the rules against *this application's designs*, not against
the `.mox` file alone: the same class may be an entity in another application.

## Example

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        value RefundDecision nonPersistent
    }
}
```

`RefundDecision` in the support domain declares `readonly decisionId`,
`readonly cents`, and `readonly approved`, so it satisfies strict value
semantics. The design says it is a value and is never persisted.

A value that is declared abstract is refused: a value has no identity to leave
abstract.

```ddd,invalid=E0364,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        abstract value RefundDecision
    }
}
```

## Related reference

- [Entities, values, and DTOs](../concepts/entities-values-and-dtos.md) — how to
  choose between the three designs.
- [Designs](../reference/designs.md) — the value rules, flags, and diagnostics.
- [The .ddd language reference](../../../../DDD.md#value-objects) — the complete
  value-semantics contract.
