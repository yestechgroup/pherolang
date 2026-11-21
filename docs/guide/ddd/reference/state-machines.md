---
title: State machines
description: Declare an entity's lifecycle with states, triggers, guards, terminals, and the rules the compiler enforces.
type: reference
language: ddd
section: reference
order: 40
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/ddd/reference/designs.md]
related: [docs/guide/ddd/how-to/define-a-state-machine.md]
lastReviewed: 2026-11-21
---

# State machines

A state machine declares the lifecycle of one enum-typed feature on an entity
design: the states that exist, the allowed moves between them, and the
conditions on those moves.

## Syntax

```text
machine <feature>: <Enum> {
    initial <State>;
    <From> -> <To> (via <Trigger>)? (when (<condition>))?;
    terminal <State>;
}
```

The block is written after the entity's repository block. `machine`,
`initial`, `terminal`, and `via` are contextual words: they remain usable as
names elsewhere.

## Semantics

- **The enum is the vocabulary.** Every state name — the initial, a terminal,
  and every transition endpoint — must be a literal of the bound enum. A typo
  is an error naming the enum's declared literals, never a new state.
- **The feature must match.** The bound feature exists on the designed class
  and carries exactly the machine's enum type.
- **A trigger is an event name.** `via Claim` names the trigger explicitly; a
  bare arrow (`InProgress -> Resolved;`) derives the trigger from the target
  state's name (`resolved`).
- **Guards are boolean.** `when (total > 0)` is typed against the designed
  class. Machine states bind as names (`state == InProgress`), never as string
  literals.
- **Operation guards share the scope.** A service operation's `when` uses the
  module's machine; in a multi-machine module, names must resolve uniquely or
  be qualified (`PayRunStatus.Draft`).

## Constraints

- Exactly one `machine` per entity design; a value or DTO cannot carry one.
- Exactly one `initial` state.
- One trigger may leave a state for at most one target (determinism).
- A terminal state has no outgoing edge; every non-terminal state has at least
  one.
- Every state is reachable from the initial state.
- Duplicate terminals and duplicate identical edges are rejected.
- A guard that does not type as boolean is an error.
- A machine state the machine never declares cannot be compared against the
  bound feature: it can never hold, so it is a type error.

## Examples

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support

    module desk {
        service TicketService {
            inject TicketRepository;
            boolean claim(String ticketNo) capability UpdateTicket when (state == Open);
            boolean resolve(String ticketNo) capability ResolveTicket;
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

`claim` can only run while the ticket is open; `Resolve` moves an in-progress
ticket to the terminal resolved state. Every state is reachable, no non-terminal
state is a dead end, and no trigger leaves a state twice.

## Diagnostics

Machine validation reports by message rather than by an allocated code. The
messages name the construct:

| Finding | Reported when |
|---|---|
| unknown state | an endpoint, initial, or terminal is not an enum literal |
| wrong enum type | the bound feature does not carry the machine's enum |
| duplicate initial / terminal | a second `initial`, or a repeated `terminal` |
| duplicate edge | the same from/to/trigger appears twice |
| non-deterministic trigger | one trigger leaves a state for two targets |
| dead end | a non-terminal state has no outgoing edge |
| unreachable state | a state cannot be reached from the initial state |
| guard is not boolean | a `when` condition types as something else |

## Compatibility

- The design model carries the machine as its bound feature, enum type, initial
  state, terminals, and transitions. The state set is derived from those, never
  stored separately.
- A bare arrow stores no trigger name; the derived trigger is the lowerCamel of
  the target state, computed by the consumer.
- The machine lowers to a statechart projection for downstream tooling; the
  design model remains the source of truth.
