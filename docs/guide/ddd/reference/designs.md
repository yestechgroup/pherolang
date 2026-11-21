---
title: Designs
description: The entity, value, and DTO declarations, their flags, invariants, aggregate boundaries, repositories, and the rules that keep each shape honest.
type: reference
language: ddd
section: reference
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/ddd/concepts/entities-values-and-dtos.md]
related: [docs/guide/ddd/reference/services-and-operations.md, docs/guide/ddd/reference/state-machines.md]
lastReviewed: 2026-11-21
---

# Designs

A design gives a domain class its semantics in this application: an entity with
identity and a lifecycle, a strictly immutable value, or a transport shape.

## Syntax

```text
entity <Class> <flags>* (root | aggregate <Root>)? invariant* repository? machine?
value  <Class> <flags>* invariant*
dto    <Class> <flags>* (from <Class> { member* } | { member* })?
```

- **Entity flags:** `scaffold`, `cache`, `auditable`, `optimisticLocking`, and
  the negations `!auditable`, `!optimisticLocking`. `auditable` and
  `optimisticLocking` are on by default for entities.
- **Boundary:** `root` authors an aggregate root; `aggregate <Root>` authors
  membership in a root designed in the same module.
- **Repeatable:** `invariant <expression>;` on an entity or value.
- **Repository:** `repository <Name> { … }` on an entity, with the built-in
  operations and declared signatures.
- **DTO forms:** mirror (`dto User`), projection (`dto BookSummary from Book {
  title; }`), standalone (`dto NewTicket { String title; }`). `nonPersistent`
  and `cache` are legal on a DTO.

## Semantics

| Shape | Identity | Mutability | Persistence | Lifecycle |
|---|---|---|---|---|
| entity | yes | mutable | repository | state machine |
| value | no | strictly immutable | none (`nonPersistent` is canonical) | none |
| dto | no | data only | never | none |

- **A value's four properties hold together.** No identity, immutable,
  deep-immutable (nested values and collections included), and structural
  equality over stored features.
- **A DTO's direction is derived.** Any derived member makes it an output
  shape; an output shape cannot fill a parameter slot.
- **Aggregates.** An authored `root` or `aggregate <Root>` outranks the
  containment derivation. An undecidable boundary warns (`E0385`) and leaves
  the design without an aggregate-root verdict.
- **Invariants** are typed boolean expressions over the designed class,
  checked by the consumer at construction, save, and transition.

## Constraints

- A value cannot carry an `id` feature (`E0361`), `contains` or `refers` toward
  a designed entity (`E0362`/`E0363`), or be `abstract` (`E0364`).
- A DTO cannot carry an operation (`E0365`), a repository, a machine, an
  entity-only flag (`E0372`), or invariants. It has no aggregate membership.
- A DTO member cannot be an entity (`E0369`); an output DTO cannot be a
  parameter (`E0370`); members and boundary names are unique (`E0366`–`E0371`).
- `root` and `aggregate <Root>` are mutually exclusive and entity-only
  (`E0379`, `E0384`); a contained class cannot be a root (`E0380`); membership
  is same-module (`E0383`).
- A repository belongs to an entity design; its declared operations take
  `throws? when?` and no capabilities, invocations, or emissions.
- Flags on the wrong shape, duplicate invariants, and a non-boolean invariant
  are errors (`E0388`/`E0389`).

## Examples

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        entity Ticket scaffold cache root repository TicketRepository {
            findById;
            findAll;
            save;
            delete;
            Ticket findByTitle(String title);
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

        dto TicketSummary from Ticket {
            ticketNo;
            title;
            state;
            refundable = refundCents > 0;
        }
    }
}
```

`Ticket` is a root entity with a scaffolded repository and a lifecycle.
`RefundDecision` is a value. `NewTicket` is a standalone input shape, and
`TicketSummary` is a projection whose derived member makes it an output shape.

## Diagnostics

| Code | Meaning |
|---|---|
| `E0361`–`E0364` | A value breaks its semantics: identity, containment of an entity, a reference to an entity, or `abstract`. |
| `E0365`–`E0372` | A DTO carries an operation, a duplicate or unknown member, an entity, becomes a parameter while output-only, collides on a name, or carries an entity-only construct. |
| `E0379`–`E0386` | Aggregate boundary contradictions: both clauses, non-entity designs, unknown or cross-module roots, contained roots, a root that is its own member. |
| `E0385` | Warning: the aggregate boundary cannot be decided from the model. |
| `E0388`, `E0389` | An invariant is invalid, or placed on a shape that cannot carry one. |

## Compatibility

- The design model stores the *effective* flag values, never the raw spelling.
- An aggregate boundary is stored as an authored or derived verdict; a design
  with no decidable verdict carries no aggregate-root flag, and consumers
  refuse it as contract-incomplete.
- A design's class identity is the domain class it names; renaming the class in
  the domain model is a new identity for every consumer.
