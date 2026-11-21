---
title: Define a state machine
description: Declare an entity's lifecycle over an enum-typed feature, with triggers, guards, terminals, and a checked reachability rule.
type: how-to
language: ddd
section: how-to
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/ddd/tutorials/your-first-application-design.md]
related: [docs/guide/ddd/reference/state-machines.md]
lastReviewed: 2026-11-21
---

# Define a state machine

Use this page when an entity moves through a fixed set of states and every
allowed move must be declared, checked, and visible to the rest of the
application.

## Problem

A `state` feature that accepts any string is not a lifecycle: nothing decides
which moves are legal, guards are scattered across service code, and a state
typographical error becomes a runtime value. You need one declaration that
names the states, the allowed edges, and the conditions on them.

## Prerequisites

- A `.ddd` design over a domain model that declares an **enum** with the
  lifecycle's states.
- The entity that carries the lifecycle designed in the same module.
- The [overview](../overview.md) read once; this page assumes the design file
  from the tutorial.

## Solution

**1. Confirm the enum exists in the domain.** The enum's literals *are* the
machine's vocabulary; the machine adds no state names of its own.

```ddd,fragment
machine status: TicketState {
```

The bound `state` feature must exist on the designed class and carry exactly
that enum type. The machine is written after the design's repository block.

**2. Declare exactly one initial state.**

```ddd,fragment
initial Open;
```

**3. Declare the edges.** Each edge is `From -> To`, optionally with
`via Trigger` and a `when (...)` guard.

```ddd,fragment
Open -> InProgress via Claim;
InProgress -> Resolved via Resolve;
```

A trigger name may leave one state for at most one target; write one edge per
choice. A bare arrow (`Draft -> Reversed;`) derives its trigger from the target
state's name.

**4. Declare the terminal states.** A terminal takes no outgoing edge; every
non-terminal state needs at least one. Every state must be reachable from the
initial state.

```ddd,fragment
terminal Resolved;
```

**5. Guard a move when a condition applies.** Guards are boolean expressions
over the designed class; machine states bind as names, never as strings.

```ddd,fragment
AwaitingApproval -> Approved via Approve when (total > 0);
```

**6. Use the states in service operations.** An operation's `when` guard types
against the module's machine: bare feature names and state names bind, with
`capability` written before `when`.

```ddd,fragment
boolean claim(String ticketNo) capability UpdateTicket when (state == Open);
```

**7. Check the design.**

```sh
pherolang check design.ddd
```

A state outside the enum, a dead end, an unreachable state, two initials, or a
non-boolean guard is reported with the line that caused it.

## Example

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        service TicketService {
            inject TicketRepository;
            boolean claim(String ticketNo) capability UpdateTicket when (state == Open);
            boolean resolve(String ticketNo) capability ResolveTicket;
            resolveTo => TicketRepository.save transition Resolve;
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

`claim` is guarded so it can only run while the ticket is open. `resolve` is
declared as an operation, and `resolveTo` delegates to the repository's `save`
while binding the `Resolve` transition, so the delegation fires the edge the
machine declares.

## Related reference

- [State machines](../reference/state-machines.md) — the exact syntax and every
  validation rule.
- [Services and operations](../reference/services-and-operations.md) — guards,
  capabilities, and delegation clauses.
- [The .ddd language reference](../../../../DDD.md#state-machines) — the
  complete engineering reference, including the statechart projection.
