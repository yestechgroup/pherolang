---
title: Use case patterns (.uc)
description: Describe one scenario as a subject, its actors, the occurrences and messages between them, their order, and the service operation that realizes it.
type: overview
language: uc
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/uc/concepts/use-case-patterns.md, docs/guide/uc/reference/use-cases-and-participants.md]
lastReviewed: 2026-11-21
---

# Use case patterns (.uc)

A `.uc` file describes one scenario of a system: who takes part, the moments
they witness, the messages they exchange, the order in which that happens, and
the application operation that carries it out. It is where a behavior
requirement becomes something the project checks.

## What is it?

A use case pattern names a **subject** — the class whose behavior the scenario
specifies — and the **actors** who take part. Inside the scenario you place
**occurrences** (moments a participant witnesses), send **messages** between
those moments, and order everything with **successions**. The subject may also
perform **steps**, each naming an operation on the subject's class.

The shape follows the use case patterns of the Systems Modeling Language
version 2. This is the Phero Lang implementation of those patterns, with the
simplifications the engineering reference lists. One warning up front, because
the word invites the wrong reading: an occurrence is a moment inside one
scenario, not an event contract. An event contract enters a scenario only as
the payload a message carries.

## What problems does it solve?

Scenario knowledge usually lives in slides, review comments, and tests. A rule
such as "the refund request reaches the ticket before the raise step begins"
ends up as a sentence in a wiki page, and the ordering is discovered to be
wrong when two operations race.

A use case pattern makes the scenario a checked part of the project. The
participants, the message ends, the order, the required capabilities, and the
operation that realizes the scenario are declared, and each named thing is
resolved against the models the project already has. A name that does not
resolve is reported before anything runs.

## Key capabilities

- **A subject and its actors.** The subject owns the specified behavior; actors
  take part either as domain-typed participants or as policy principals.
- **Occurrences.** Name a lifetime point a participant witnesses, or own one
  end of a message.
- **Messages with typed payloads.** A message carries a domain type or a
  declared event's name, and attaches to an occurrence or a step at each end.
- **Ordering.** `first … then …` successions state what completes before what
  begins, with `start` and `done` implicit.
- **Performed steps.** The subject's steps name domain operations, optionally
  guarded by a boolean condition over the subject.
- **Required capabilities.** `requires` names the capabilities the participating
  actors must hold, and a scenario that needs two capabilities a policy forbids
  together is refused.
- **Realization.** `realizes` ties a scenario to the application service
  operation that implements it.

## A simple example

A ticket is clarified by its reporter. The subject witnesses the moment the
clarification is requested, the reporter witnesses sending it, and one message
joins the two:

```uc,fixture=examples/support
import "support.mox"

usecase ClarifyTicket {
    subject Ticket: Ticket {
        occurrence clarificationRequested;
    }
    actor Reporter: User {
        occurrence clarificationSent;
    }
    message requestInfo of Ticket from Ticket.clarificationRequested to Reporter.clarificationSent;
    first start then requestInfo.source;
    first requestInfo.source then requestInfo;
    first requestInfo then done;
}
```

`Ticket` is the subject and `Reporter` is a domain-typed actor. The message
leaves at the end of `Ticket.clarificationRequested` and arrives at the start of
`Reporter.clarificationSent`; the successions place the sending end, the
receiving end, and `done` in order. A bare `requestInfo` in a succession means
the message's receiving end, which is why both ends are reachable from `start`.

## How it fits into Phero Lang

A use case pattern is assembled from the other models rather than defining its
own types:

- `.mox` supplies every type: the subject's class, typed actors, message
  payloads, and the operations steps name. See
  [domain models](../mox/overview.md).
- `.ddd` supplies the application service operations a `realizes` line names.
  See [application designs](../ddd/overview.md).
- `.actor` supplies the policy principals that bare actors name, and the
  capabilities `requires` lists.
- `.evt` supplies event names usable as message payloads. Nothing else about
  events appears in a scenario.
- `.ifml` describes the screens and navigation through which people meet the
  scenario; the use case names the behavior behind them. See
  [interaction flows](../ifml/overview.md).
- `.cmmn` models work that cannot be planned in advance; a use case pattern
  describes one planned scenario instead. See
  [case models](../../CMMN.md).
- `.dmn` supplies decisions the realizing operations may invoke. See
  [decision models](../dmn/overview.md).

The `support` project under `examples/` uses all four kinds of import in one
file: a domain, an application design, an actor policy, and an event contract.

## Getting started

There is no tutorial for this language yet. Start with the concept page, then
read the reference for the exact rules:

- [Use case patterns](concepts/use-case-patterns.md) walks through the parts of
  a scenario and how they compose.
- [Use cases and participants](reference/use-cases-and-participants.md) is the
  syntax and validation reference.
- The [support-desk use case example](../../../examples/support/support.uc) is
  a complete file with bare policy actors, message-owned ends, steps, and
  `realizes` bindings.

## Further reading

- [Use case patterns](concepts/use-case-patterns.md) — participants,
  occurrences, messages, and successions as one idea.
- [Use cases and participants](reference/use-cases-and-participants.md) — the
  statement forms and the constraints on each.
- [The .uc language reference](../../USECASES.md) — the complete engineering
  reference, including the import rules, validation order, and the deliberate
  differences from the standard.
