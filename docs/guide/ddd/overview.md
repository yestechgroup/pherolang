---
title: Application design (.ddd)
description: Describe the application built on a domain model — its modules, services, repositories, entity lifecycles, value objects, and transport shapes.
type: overview
language: ddd
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/ddd/concepts/entities-values-and-dtos.md, docs/guide/ddd/reference/designs.md]
lastReviewed: 2026-11-21
---

# Application design (.ddd)

A `.ddd` file describes the application built on top of a domain model: how the
domain is divided into modules, which services expose use cases, which entities
own repositories, how an entity's lifecycle moves, and which shapes cross the
application's boundaries.

## What is it?

A domain model says what the domain *is*. An application design says how the
application *uses* it. The design names the application, groups its work into
modules, declares the services and their operations, decides which domain
classes are entities with identity and lifecycle and which are immutable
values, and defines the transport shapes that enter and leave the application.

The domain classes themselves are not redeclared. A design is written against
an imported `.mox` model: `entity Ticket` says what the already-declared
`Ticket` class means to this application, not what fields it has. The same class
can be an entity in one application and a value in another, because each design
states its own semantics.

## What problems does it solve?

A domain model cannot answer every design question. It cannot say that a ticket
must be resolved through a specific sequence of steps, that refund decisions
never change once recorded, that ticket creation accepts a small input shape
rather than the whole entity, or that listing tickets goes through a repository
with one declared finder.

Without a place for those decisions, they scatter into service code, database
schemas, and API adapters, and stop being reviewable together. A `.ddd` design
collects them in one file that is checked as a whole: if a lifecycle state is
not in its enum, or a value object carries an identity, or an operation throws
an error it did not declare, the design does not compile.

## Key capabilities

- **Applications and modules.** Name the application, group its designs by
  module, and import the domain model the design is built on.
- **Services.** Declare use-case operations with typed parameters, return
  types, capabilities, emitted events, decisions they invoke, guards, and
  errors they can raise.
- **Entities and repositories.** Give a domain class identity and a lifecycle,
  and attach the repository that stores it with built-in and declared
  operations.
- **Value objects.** Design a class as strictly immutable, with no identity and
  structural equality.
- **Transport shapes.** Design a DTO as a mirror, a projection, or a
  standalone input shape, with its direction derived from what it selects.
- **State machines.** Declare a lifecycle over an enum-typed feature with
  triggers, guards, terminals, and reachability rules.
- **Multi-file applications.** Split a large design across a root and module
  fragments that merge into one application.

## A simple example

A support-desk application over the support domain: one service, one entity
with a repository, one lifecycle, one value, and one input shape.

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        service TicketService {
            inject TicketRepository;
            Ticket getTicket(String ticketNo);
            boolean claimTicket(String ticketNo);
            boolean resolveTicket(String ticketNo);
        }

        entity Ticket repository TicketRepository {
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
    }
}
```

`SupportDesk` names the application; `desk` groups its designs. `Ticket` is an
entity stored through `TicketRepository`; its lifecycle moves from open to
in progress to resolved. `RefundDecision` is an immutable value. `NewTicket` is
the shape accepted when creating a ticket, rather than the entity itself.

## How it fits into Phero Lang

`.ddd` imports a `.mox` domain model — one or more packages — and designs over
it. The design is then consumed by the rest of the ecosystem:

- `.deploy` binds an application design to runtime targets and profiles.
- `.evt` event contracts describe the events an operation emits.
- `.dmn` decision models provide the decisions an operation invokes.
- `.ifml` interaction flows describe the screens and navigation in front of
  the services.
- `.uc` use case patterns relate actors and system behavior across the
  application.
- `.cmmn` case models handle the work that cannot be fully planned.

The domain model is the stable center; the design adds the application's
decisions without duplicating the domain.

## Getting started

- [Tutorial — your first application design](../tutorials/your-first-application-design.md)
  builds the example above from an empty file, one step at a time.
- [Define a state machine](../how-to/define-a-state-machine.md) goes deeper on
  lifecycles and guards.
- [Define a value object](../how-to/define-a-value-object.md) covers strict
  value semantics.
- [Write an operation body](../how-to/write-an-operation-body.md) shows the
  statement subset an operation can execute.

## Further reading

- [Entities, values, and DTOs](../concepts/entities-values-and-dtos.md) explains
  the three design shapes and when each applies.
- [Relationship with Sculptor](../concepts/relationship-with-sculptor.md) maps
  the concepts Phero Lang shares with Sculptor, and the ones it deliberately
  changes.
- [Designs](../reference/designs.md), [services and operations](../reference/services-and-operations.md),
  and [state machines](../reference/state-machines.md) are the reference pages.
- [The .ddd language reference](../../../DDD.md) is the complete engineering
  reference, including the wire contract and multi-file resolution.
