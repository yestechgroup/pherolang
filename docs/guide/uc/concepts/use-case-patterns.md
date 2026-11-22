---
title: Use case patterns
description: How a scenario is written down — one subject, its actors, the occurrences they witness, the messages between them, and their order.
type: concept
language: uc
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/uc/overview.md]
related: [docs/guide/uc/reference/use-cases-and-participants.md]
lastReviewed: 2026-11-21
---

# Use case patterns

A use case pattern is one scenario written as declarations: a subject whose
behavior the scenario specifies, the actors taking part, the moments each of
them witnesses, the messages exchanged between those moments, and the order in
which it all happens.

## Overview

The pattern has five parts, and each part answers one question:

| Part | Written as | Answers |
|---|---|---|
| Subject | `subject <name>: <Type>` | Whose behavior does this scenario specify? |
| Actor | `actor <name>` or `actor <name>: <Type>` | Who else takes part? |
| Occurrence | `occurrence <name>;` | Which moment does a participant witness? |
| Message | `message <name> of <Type> from <p>.<a> to <p>.<a>;` | What is sent, from where, to where? |
| Succession | `first <node> then <node>;` | What completes before what begins? |

Two more declarations complete the pattern: a **step** performed by the
subject, which names an operation on the subject's class, and a **realizes**
binding, which names the application service operation that implements the
scenario.

A use case pattern is not an event contract. It is also not a general-purpose
sequence diagram: it has no loops, no alternatives, and no nesting. It states
one scenario and the order inside it.

## Why does it matter?

Ordering and participation are the parts of a requirement that code reviews
rarely check. "The recipient acknowledges before the drone releases the parcel"
is easy to write in a document and easy to violate in a service operation,
because nothing in the codebase mentions the promise.

Declaring the scenario gives that promise a home. Message ends must attach to a
declared moment, so a message cannot be sent from nowhere. Steps must name real
operations, so the scenario cannot drift away from the services it describes.
Required capabilities must be held by a participating actor, so a scenario
cannot quietly assume a capability nobody granted. Every one of those checks
runs when the project is checked, not when the race is lost.

The second cost of leaving scenarios in prose is traceability. A `realizes`
line is a link from the requirement to the operation that implements it, which
makes the question "which scenario does this operation serve?" a lookup rather
than an archaeology exercise.

## How it works

**Participants.** A use case declares at most one `subject`: the class whose
behavior the scenario specifies. The subject is the only participant that
performs steps. Actors come in two forms. A typed actor —
`actor Reporter: User` — is an instance of a domain class and is used for
participants the domain model already describes. A bare actor —
`actor Courier` — is a policy principal: its name must match an actor declared
in an imported actor policy, and it is how the scenario takes part in
capability checks. A scenario may declare actors without a subject when the
point is who takes part, not what happens.

**Occurrences.** An occurrence is a scenario-local lifetime point: a named
moment one participant witnesses. It can be declared directly
(`occurrence clarificationRequested;`) or as one end of a message
(`occurrence releaseParcel.source;`, `occurrence releaseParcel.target;`). The
second form says "this participant owns that end of that message". Both forms
are moments and nothing more — there is no payload, no channel, and no
subscription. This is why an occurrence is not an event: an event contract is a
statement about the system that outlives any one scenario, while an occurrence
exists only inside the scenario that declares it. Event contracts reach a
scenario solely as message payloads.

**Messages.** A message is a named communication with an optional payload and
two ends. Each end is an anchor — `participant.anchor` — naming an occurrence or
a step of that participant. The rule that gives messages their meaning is
temporal: a message is sent at the end of its source anchor and received at the
start of its target anchor. Sending therefore completes before the message
leaves, and the receiving anchor begins only after it arrives. Each end is
bound exactly once: either by an inline endpoint on the message or by exactly
one participant declaring `occurrence <message>.<side>`, never both, never
neither. A payload names a domain type first; failing that, it names a declared
event, which is the only way an event contract enters a scenario.

**Successions.** A succession states that one node completes before another
begins. `start` and `done` are the scenario's implicit first and last nodes.
A node can be a step name, a message end (`m.source` or `m.target`), a
participant's occurrence or step (`participant.x`), or a bare message name,
which means that message's receiving end — the shorthand for "the message has
arrived". When a scenario declares any succession, the whole graph must be
acyclic, every step and message end must be reachable from `start`, and `done`
must be reachable. A scenario with no succession makes no ordering promise and
skips those checks.

**Steps.** A step names an operation the subject performs. The operation must
exist on the subject's class, and the step may carry a `requires` list of
capabilities and a `when` guard typed as boolean over the subject. A step is
not a call the scenario executes; it is the point at which the named operation
participates in the order.

**Imports.** A `.uc` file imports from four kinds of source, and the kind is
decided by what the imported file is, not by the line: a domain model supplies
types, an application design supplies service operations for `realizes`, an
actor policy supplies principals and capabilities, and an event contract
supplies payload names. A design import may carry an alias, which is what a
`realizes` line names; aliases on any other kind are an error.

## Example

A ticket performs the raise step, the reporter witnesses the request, and one
message carries the request to the step:

```uc,fixture=examples/support
import "support.mox"

usecase RaiseRefund {
    subject Ticket: Ticket {
        perform raise: raiseRefund;
    }
    actor Reporter: User {
        occurrence refundRequested;
    }
    message requestRefund of Ticket from Reporter.refundRequested to Ticket.raise;
    first start then requestRefund.source;
    first requestRefund.source then requestRefund;
    first requestRefund then raise;
    first raise then done;
}
```

`Reporter` is a domain-typed actor; `refundRequested` is the moment the
reporter asks. The message's payload is the ticket, its source anchor is that
occurrence, and its target anchor is the subject's `raise` step — so the raise
step begins after the request arrives. The last succession closes the scenario
at `done`. Note what is absent: no event is declared, no channel is named, and
no code is written. The scenario says who, when, and in what order.

## Relationship to other concepts

- A [domain model](../../mox/overview.md) supplies the subject's class, typed
  actors, payload types, and the operations steps name.
- An [application design](../../ddd/overview.md) supplies the service
  operations `realizes` binds to.
- An actor policy, written in the [language reference](../../../LANGUAGE.md),
  supplies bare actor principals and the capabilities `requires` lists.
- An [event contract](../../../EVENTS.md) supplies message payload names, and
  nothing else; occurrences are not events.
- An [interaction flow](../../ifml/overview.md) describes the screens through
  which people meet the scenario.
- A [case model](../../../CMMN.md) describes adaptive work whose path cannot be
  planned; a use case pattern describes one planned path.
- A [decision model](../../dmn/overview.md) supplies decisions the realizing
  operations can invoke.

## Further reading

- [Use cases and participants](../reference/use-cases-and-participants.md) —
  the statement forms, the constraints, and the model the compiler produces.
- [The .uc language reference](../../../USECASES.md) — import resolution, the
  validation rules in order, and the deliberate differences from the standard.
- The [support-desk use case example](../../../../examples/support/support.uc) —
  one file using all four import kinds.
