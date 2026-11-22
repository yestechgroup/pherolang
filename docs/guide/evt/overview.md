---
title: Event contracts (.evt)
description: Declare the events an application publishes, the channels that carry them, and the subscriptions that deliver them to named consumers.
type: overview
language: evt
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/evt/concepts/events-channels-and-subscriptions.md, docs/guide/evt/reference/event-contracts.md, docs/guide/evt/how-to/declare-an-event-contract.md]
lastReviewed: 2026-11-21
---

# Event contracts (.evt)

A `.evt` file declares what an application publishes: the events themselves,
the channels that carry them, and the subscriptions that deliver them to named
consumers. It is the agreement between a producer and the people who react to
its messages — one file that says what happened, what data the message carries,
and who receives it.

## What is it?

An event contract is a declaration of message flow. It has three parts:

- An **event** is a named contract with typed payload members and a version
  label.
- A **channel** lists the events published on one logical stream.
- A **subscription** names one consumer, the events that consumer receives, and
  optionally a filter and a delivery policy.

The payload types come from an imported `.mox` domain model. The event declares
no types of its own, so a payload member is a domain class, enum, datatype, or
primitive — the same type the rest of the application uses.

The file says nothing about brokers or message encoding. A consumer chooses the
transport; the contract only states what is published and who subscribes.

## What problems does it solve?

Most systems describe their events three times: in the code that publishes
them, in the code that consumes them, and in a document that is already out of
date. The descriptions drift, and the drift shows up as a message a consumer
cannot read.

A second problem is quieter: nobody can answer "who receives this event, and
with what guarantee?". Routing lives in configuration, filters live in consumer
code, and the retry policy lives in a broker setting that no reviewer sees.

An event contract makes both explicit. The event names the payload and its
version. The channel states membership. The subscription states the consumer,
the filter, and the delivery policy. Every payload type is checked against the
domain model, so a misspelled type is a compile error rather than a message
that fails at runtime.

## Key capabilities

- **Typed payloads.** Every payload member resolves against the imported domain
  model — classes, enums, datatypes, and primitives.
- **Event versions.** Each event carries a version label, stored exactly as
  written.
- **Channels.** A channel groups the events published on one logical stream and
  can name the scope its topics are parameterized by.
- **Subscriptions.** One consumer, a list of events, an optional filter, and an
  optional delivery policy.
- **Delivery declarations.** Delivery mode, attempt and timeout settings, and a
  dead-letter channel travel with the subscription.
- **Governance metadata.** An event can declare its owner, classification,
  retention period, and the payload members that carry personal data.
- **CloudEvents declarations.** An event can declare the CloudEvents attributes
  a generator should use instead of deriving them.

## A simple example

A ticket-resolved event, the channel that publishes it, and one subscriber:

```evt,fixture=examples/support
import "support.mox"

event TicketResolved version "1.0.0" {
    ticketNo: String;
    state: TicketState;
}

channel tickets {
    publishes TicketResolved;
}

subscription reporting {
    events [ TicketResolved ]
    consumer reporting_service
}
```

`state: TicketState` resolves to the enum the imported domain declares.
`tickets` publishes the event, and `reporting` is one consumer of it. A second
subscriber needs another `subscription` block, not a change to the event or the
channel.

## How it fits into Phero Lang

- `.mox` supplies every payload type. See
  [Domain models](../mox/overview.md).
- `.ddd` service operations produce events with their `emits` clause. See
  [Application designs](../ddd/overview.md).
- `.dmn` decisions can map an outcome to declared events with their `emits`
  block. See [Decision models](../dmn/overview.md).
- `.deploy` declares the components a subscription's consumer resolves to
  during a project check. See [Deployment models](../../DEPLOY.md).
- `.intent` may link a requirement to a declared event as the element that
  expresses it. See [Intent specifications](../../INTENTS.md).

A project check ties the layers together: every event an application operation
emits must be declared in a contract, and every declared event must be emitted
by an application operation. A decision's `emits` block is checked against a
contract that the project provides. A contract checked on its own stays silent
about all of that wiring.

## Getting started

- [Declare an event contract](how-to/declare-an-event-contract.md) builds a
  checked contract from a domain model step by step.
- [Event contracts](reference/event-contracts.md) is the reference for every
  declaration and validation rule.
- The [support-desk event contract](../../../examples/support/support.evt) is a
  complete file the project's tests compile on every change.

## Further reading

- [Events, channels, and subscriptions](concepts/events-channels-and-subscriptions.md)
  explains why the three parts are separate.
- [Event contracts](reference/event-contracts.md) states the syntax,
  semantics, and constraints.
- [The .evt engineering reference](../../EVENTS.md) covers the compiled model
  and its consumers.
