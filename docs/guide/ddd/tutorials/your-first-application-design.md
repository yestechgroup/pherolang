---
title: Tutorial — your first application design
description: "Design the application around an existing domain model: modules, an entity with a repository, a service, a lifecycle, a value, and an input shape."
type: tutorial
language: ddd
section: tutorials
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/mox/overview.md]
related: [docs/guide/ddd/how-to/define-a-state-machine.md, docs/guide/ddd/reference/designs.md]
lastReviewed: 2026-11-21
---

# Tutorial — your first application design

A domain model describes the data; an application design decides how the
application uses it. In this tutorial you write a design over the support-desk
domain that ships with the project, and check it at every step.

## What you'll learn

- Name an application and import the domain model it designs over.
- Design a domain class as an entity with a repository.
- Declare a service and its operations.
- Model a lifecycle with a state machine.
- Design a value object and an input shape.
- Check the design and read its diagnostics.

## Prerequisites

- Phero Lang installed, with `pherolang --version` working.
- The repository checked out; this tutorial uses
  `examples/support/support.mox` as its domain.
- The [application design overview](../overview.md) read once.

## What we're building

One file, `design.ddd`, next to a copy of `support.mox`. When it is done,
`pherolang check design.ddd` prints `OK design.ddd`.

Create a working directory and copy the domain into it:

```sh
mkdir support-design
cd support-design
cp <path-to-repository>/examples/support/support.mox .
```

The domain declares `Ticket` (with `ticketNo`, `title`, `refundCents`, and a
`TicketState`), `User`, and `RefundDecision`. You will not redeclare any of
them: the design says what they mean to this application.

## Step 1 — Name the application and import the domain

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support
    module desk {
    }
}
```

`import` resolves relative to the design file, so `support.mox` must sit next
to `design.ddd`. `base` names the domain package the design is built on.
`module desk` groups the designs; an empty module is valid, which is why this
step checks cleanly.

```sh
pherolang check design.ddd
```

You should see `OK design.ddd`.

## Step 2 — Design the ticket as an entity

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
    }
}
```

`entity Ticket` gives the domain class identity semantics in this application.
The `repository` block attaches persistence: the four listed operations are
built-in repository operations every consumer understands.

```sh
pherolang check design.ddd
```

Still `OK design.ddd`.

## Step 3 — Declare a service

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
        }
    }
}
```

`inject TicketRepository` makes the repository available to the service. The
operations declare typed parameters and return types; the service is the
application's public surface for the ticket queue.

```sh
pherolang check design.ddd
```

## Step 4 — Give the ticket a lifecycle

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support
    module desk {
        service TicketService {
            inject TicketRepository;
            boolean claimTicket(String ticketNo);
            boolean resolveTicket(String ticketNo);
        }

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
    }
}
```

The machine binds to the entity design that precedes it and to the
`TicketState` enum the domain declares. Every state name is an enum literal:
`Open`, `InProgress`, and `Resolved` already exist, so the machine adds no
vocabulary. A trigger names the event that moves the ticket; `Resolved` is
declared terminal, so it takes no outgoing edge.

```sh
pherolang check design.ddd
```

## Step 5 — Add a value object and an input shape

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

`value RefundDecision` says the refund decision is immutable: the domain's
features are `readonly`, so the class satisfies strict value semantics.
`NewTicket` is a standalone DTO — the shape a caller sends when creating a
ticket — so the service never has to accept a fully formed entity.

```sh
pherolang check design.ddd
```

## Step 6 — Put it together

The complete file is the [overview's example](../overview.md). Add the service
from step 3 back next to the entity, value, and DTO, then check it one last
time.

```sh
pherolang check design.ddd
```

## Complete example

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

## Expected result

```sh
$ pherolang check design.ddd
OK design.ddd
```

The command exits with status 0. The design is one application named
`SupportDesk` with one module, one service, one entity design with its
repository and lifecycle, one value design, and one DTO.

## Next steps

- [Define a state machine](../how-to/define-a-state-machine.md) adds guards and
  transition bindings to the lifecycle.
- [Write an operation body](../how-to/write-an-operation-body.md) turns
  `resolveTicket` from a declaration into a checked procedure.
- [Designs](../reference/designs.md) lists every flag and constraint the
  entity, value, and DTO shapes carry.
- The [support-desk example](../../../../examples/support/support.ddd) is the
  full design this tutorial builds toward, including searches, projections,
  and executable bodies.
