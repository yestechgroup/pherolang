---
title: Relationship with Sculptor
description: Where Phero Lang's application design comes from, which Sculptor concepts it keeps, and which it deliberately changes.
type: concept
language: ddd
section: concepts
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md]
related: [docs/guide/ddd/concepts/entities-values-and-dtos.md, docs/guide/ddd/reference/designs.md]
lastReviewed: 2026-11-21
---

# Relationship with Sculptor

Phero Lang's application design is inspired by Sculptor, the domain-driven
design framework. The vocabulary is recognisable on purpose: entities, value
objects, repositories, services, modules, and DTOs all mean roughly what a
Sculptor user expects. The differences are deliberate, and knowing where they
are prevents a reader from importing assumptions that do not hold.

## Overview

This page states the relationship explicitly. Sculptor is a **design lineage**,
not a specification Phero Lang claims to conform to: the mapping below
separates the concepts Phero Lang implements, the extensions it adds, and the
places where it deliberately diverges.

| Classification | Meaning here |
|---|---|
| Phero implementation | A Sculptor concept Phero Lang carries with the same intent. |
| Phero extension | Something Phero Lang adds beyond Sculptor's shape. |
| Phero divergence | A deliberate difference; the Sculptor spelling is not accepted. |

## Why does it matter?

A reader who knows Sculptor will otherwise look for `!immutable`, `fetch`,
`cascade`, `orderby`, or `Bag`, and conclude the design is incomplete when they
are absent. They are absent because the modelling choices behind them were
rejected, not because the language forgot them. Stating the difference keeps
the design reviewable: a proposal to add one of them is a proposal to reverse a
documented decision.

## How it works

**Implemented with the same intent.** Entity, value object, repository,
service, module, DTO, and the design flags (`scaffold`, `auditable`,
`optimisticLocking`, `cache`) are Phero implementations. A repository carries
Sculptor's built-in operations plus declared signatures; a service injects
repositories and delegates operations with `=>`; a DTO has mirror, projection,
and standalone forms.

**Extended beyond Sculptor.** Declared repository **queries** replace string
finders with a typed predicate, sort, and pagination. Full-text **search** is a
declaration, not an engine-specific query. State machines are part of the
design, with guards and a checked reachability rule. `children` blocks declare
operations on an aggregate root's contained children. Design **invariants** are
typed boolean predicates rather than validator hooks.

**Deliberately different.** A value object is strictly immutable: there is no
`!immutable` opt-out. Collections carry their kind on the domain feature, not
in a signature, so there is no `Set`/`List`/`Bag` in a parameter. `fetch`,
`cascade`, `orderby`, and `orderColumn` are absent because persistence
behavior belongs to the consumer. A `Map` is a deliberate non-goal. Aggregate
boundaries are authored with `root`/`aggregate <Root>` or derived from
containment, not with `belongsTo`/`!aggregateRoot`.

## Example

A design in the shared vocabulary: an entity, its repository, and a value.

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

        value RefundDecision nonPersistent
    }
}
```

This is the part a Sculptor reader recognises immediately. What they will not
find is a mutable value or a collection kind on a signature: those spellings
are refused, and the design says so at compile time.

## Relationship to other concepts

- [Entities, values, and DTOs](entities-values-and-dtos.md) states what each
  shape promises; this page states where those promises came from.
- [Designs](../reference/designs.md) is the authoritative list of flags and
  constraints, including the ones Sculptor does not have.
- The engineering reference's [Sculptor concept
  mapping](../../../../DDD.md#sculptor-concept-mapping) carries the complete
  table, including the rationale for every divergence.

## Further reading

- [The .ddd language reference](../../../../DDD.md#deliberate-divergences) — the
  detailed reasons behind each rejected concept.
- [State machines](../reference/state-machines.md) — one of the extensions in
  depth.
- [Designs](../reference/designs.md) — aggregate boundaries as Phero Lang
  authors them.
