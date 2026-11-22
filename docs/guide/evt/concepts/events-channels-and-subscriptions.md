---
title: Events, channels, and subscriptions
description: How an event contract separates what happened, where it is published, and who receives it, and what each part promises.
type: concept
language: evt
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/evt/overview.md]
related: [docs/guide/evt/reference/event-contracts.md, docs/guide/evt/how-to/declare-an-event-contract.md]
lastReviewed: 2026-11-21
---

# Events, channels, and subscriptions

An event contract has three declaration kinds, and each answers a different
question. An event says what happened. A channel says where it is published. A
subscription says who receives it, and under what conditions.

## Overview

An **event** is a named message with typed payload members and a version label.
It is the unit a producer creates and a consumer reads.

A **channel** is a named stream that lists the events published on it. A channel
is membership, nothing more: it does not say who listens, and it does not choose
a transport.

A **subscription** is a delivery rule. It names one consumer, the set of events
that consumer receives, an optional filter, and an optional delivery policy. A
subscription is how a new consumer joins the system without changing the
producer.

The three are separate on purpose. Adding a consumer is a subscription. Adding
a new kind of message is an event, plus a `publishes` entry on the channel that
carries it. Changing a retry policy is a delivery setting on the subscription
that needs it.

## Why does it matter?

When a producer owns its consumers' addresses, every new consumer is a change
to the producer, and every changed address is a release. When a consumer owns
its filter in code, the filter is invisible to the people who route messages.
When a retry policy lives only in broker settings, no reviewer of the
application can see it.

Separating the three parts keeps each change local:

- A subscriber is added, changed, or removed without touching the event or the
  channel.
- Two subscribers with different needs read the same event. One may filter it;
  the other may take everything.
- The delivery guarantee is written next to the consumer that relies on it.

The separation also makes the contract checkable. A channel that publishes an
event nobody declares is an error, and a subscription that lists an undeclared
event is an error. Nothing about routing has to be inferred from code.

## How it works

**Declarations reference each other by name.** All three kinds live in one
file, and every reference must name a declaration in that file. A `publishes`
entry names an event; a subscription's `events` list names events; a
`deadLetter` entry names a channel.

**Payload types resolve against the domain.** The contract imports one or more
`.mox` models. Each payload member's type is then resolved in the union of those
models, following the same rules a multi-file domain model uses. A payload
member carries exactly one value of its declared type: an event has no
multiplicity syntax, so a collection is modeled by referencing a class that
holds it.

**The version label is yours to define.** `version "1.0.0"` is stored exactly as
written. The examples use the familiar `X.Y.Z` shape, but the language attaches
no ordering or comparison to the text, so the meaning of a version change is a
decision for the producer and its consumers.

**A filter is a routing decision.** A subscription's `when` condition is typed
against every event in its list. That is deliberate: the same condition must be
able to decide, for each event that arrives, whether this consumer wants it. A
payload member named in the filter must therefore exist on every subscribed
event.

**Scope labels are opaque.** A channel can declare `of <scope>` and a
subscription can bind `scope <scope>`. The labels are carried as written; the
language never resolves or compares them. They identify the topic dimension a
transport parameterizes, which is the transport's concern.

**Delivery is declared, not run.** A delivery policy states the mode
(`at_most_once`, `at_least_once`, or `effectively_once`), optional attempt,
backoff, and timeout settings, and an optional dead-letter channel. A consumer
that implements delivery reads the declaration; the contract does not execute it
and names no broker.

## Example

Two subscribers over one event set. `refundDesk` filters to positive amounts;
`analytics` takes every request:

```evt,fixture=examples/support
import "support.mox"

event RefundRequested version "1.1.0" {
    ticketNo: String;
    refundCents: int;
}

event RefundApproved version "1.0.0" {
    ticketNo: String;
    refundCents: int;
}

channel refunds {
    publishes RefundRequested;
    publishes RefundApproved;
}

subscription refundDesk {
    events [ RefundRequested RefundApproved ]
    consumer refund_approval_service
    when (refundCents > 0)
}

subscription analytics {
    events [ RefundRequested ]
    consumer analytics_service
}
```

The `refunds` channel publishes both events. `refundDesk` receives both and
filters on a payload member they share. `analytics` receives only the request
event and needs no filter. Neither subscription changes the events, and the
channel does not know that either consumer exists.

## Relationship to other concepts

- The [domain model](../../mox/overview.md) supplies every payload type. A
  class-typed payload refers to the class; for an entity that means its
  identity, not a copy of its state.
- An [application operation](../../ddd/reference/services-and-operations.md)
  produces an event through its `emits` clause. That clause is the production
  side of the contract.
- A [decision](../../dmn/reference/decision-models.md) can map an outcome to
  declared events through its `emits` block.
- A deployment model declares the components a subscription's consumer
  resolves to during a project check; the
  [.deploy engineering reference](../../../DEPLOY.md) covers that surface.
- The [reference](../reference/event-contracts.md) states the syntax and every
  validation rule.

## Further reading

- [Declare an event contract](../how-to/declare-an-event-contract.md) applies
  these parts to a working file.
- [Event contracts](../reference/event-contracts.md) is the authority for the
  declaration forms.
- [The .evt engineering reference](../../../EVENTS.md) describes the compiled
  model and the declarations consumers receive.
