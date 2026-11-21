---
title: Write an operation body
description: Turn a declared service operation into a checked procedure using the closed statement subset — loads, guards, raises, assignment, and return.
type: how-to
language: ddd
section: how-to
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/ddd/tutorials/your-first-application-design.md]
related: [docs/guide/ddd/reference/services-and-operations.md]
lastReviewed: 2026-11-21
---

# Write an operation body

Use this page when a use case is more than a single delegation: it must load an
entity, refuse an invalid state, change the entity, and save it. An operation
body states that sequence in the design, and the compiler checks every step.

## Problem

Once a use case needs a guard and an error, a signature alone cannot say what
the operation does. The rules end up in hand-written service code, where they
are not checked against the domain types, and where the error vocabulary drifts
from what callers handle.

## Prerequisites

- The entity designed with a repository in the same module.
- The repository injected into the service.
- Any error the body raises declared in the design's vocabulary (`error
  NotFound` is part of the implicit prelude).
- The [overview](../overview.md) read once.

## Solution

**1. Write the signature with the errors it can raise.**

```ddd,fragment
boolean resolveTicket(String ticketNo) throws NotFound, TicketAlreadyResolved => TicketRepository {
```

The name after `=>` is the repository the body binds as `self`. Every other
injected repository is available by its injection name. `throws` is explicit:
a raise outside the declared set is an error, never inferred.

**2. Load what the operation needs.**

```ddd,fragment
let ticket = self.findById(ticketNo) else error NotFound("Ticket", ticketNo);
```

`findById` returns an optional; `let … else error …` handles the empty case
with a declared error.

**3. Refuse invalid state with a guard.**

```ddd,fragment
guard ticket.state != TicketState.Resolved else error TicketAlreadyResolved(ticketNo);
```

**4. Change the entity, save it, and return.**

```ddd,fragment
ticket.state = TicketState.Resolved;
let saved = self.save(ticket);
return true;
```

**5. Check the design.**

```sh
pherolang check design.ddd
```

An unknown error, a missing return, a statement outside the subset, or a
repository the service does not inject is reported with its line.

## Example

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support
    error TicketAlreadyResolved(String ticketNo);

    module desk {
        service TicketService {
            inject TicketRepository;
            boolean resolveTicket(String ticketNo) throws NotFound, TicketAlreadyResolved => TicketRepository {
                let ticket = self.findById(ticketNo) else error NotFound("Ticket", ticketNo);
                guard ticket.state != TicketState.Resolved else error TicketAlreadyResolved(ticketNo);
                ticket.state = TicketState.Resolved;
                let saved = self.save(ticket);
                return true;
            }
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

The body is **orchestration-only**: it may declare locals, assign to a loaded
local's feature, guard, branch with `if`/`else`, and return. Loops, local or
compound reassignment, collection mutation, `try`/`catch`, cross-aggregate
loads, and bare expression statements are refused by name — those belong in a
domain operation or in target code, not in the design.

An error outside the declared vocabulary is refused:

```ddd,invalid=E0373,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        service TicketService {
            inject TicketRepository;
            boolean closeTicket(String ticketNo) throws Mystery => TicketRepository {
                let ticket = self.findById(ticketNo) else error NotFound("Ticket", ticketNo);
                return true;
            }
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

## Related reference

- [Services and operations](../reference/services-and-operations.md) — the
  operation grammar, clause order, and every body rule.
- [Designs](../reference/designs.md) — entity and repository declarations.
- [The .ddd language reference](../../../../DDD.md#the-executable-body-posture)
  — the complete body contract, including the error vocabulary rules.
