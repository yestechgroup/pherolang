---
title: Declare an event contract
description: Write a .evt file that declares typed events, a channel, and a subscription, and check it against the domain model.
type: how-to
language: evt
section: how-to
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/evt/overview.md]
related: [docs/guide/evt/concepts/events-channels-and-subscriptions.md, docs/guide/evt/reference/event-contracts.md]
lastReviewed: 2026-11-21
---

# Declare an event contract

Use this page when you have a domain model and need to state what the
application publishes and who receives it. When you are done, the contract
checks cleanly against the domain and names one consumer per delivery rule.

## Problem

The events an application publishes are usually implicit: a producer builds a
message in code, a consumer parses it by convention, and nothing checks that the
two agree. You need one file that declares the event payloads, the channel they
travel on, and the subscriber that receives them, so the compiler can check the
types and the references before anything runs.

## Prerequisites

- A domain model (`.mox`) with the classes, enums, and primitives your payloads
  use. The [domain-model overview](../../mox/overview.md) covers the language.
- The `pherolang` command available in your shell.
- [Event contracts](../overview.md) read once, and
  [events, channels, and subscriptions](../concepts/events-channels-and-subscriptions.md)
  for why the parts are separate.

## Solution

**1. Create the contract next to the domain and import it.** The import path is
relative to the contract's directory and has no trailing semicolon.

```evt,fragment
import "support.mox"
```

**2. Declare the event.** Name it for the fact that happened, give it a version
label, and type each payload member against the domain.

```evt,fragment
event TicketRaised version "1.0.0" {
    ticketNo: String;
    title: String;
    state: TicketState;
}
```

**3. Declare a channel and publish the event on it.** A `publishes` entry names
an event in the same file; the channel adds no transport.

```evt,fragment
channel tickets {
    publishes TicketRaised;
}
```

**4. Declare a subscription.** One consumer, one event list. The consumer is a
name; a project check resolves it to a declared deployment component.

```evt,fragment
subscription triage {
    events [ TicketRaised ]
    consumer triage_service
}
```

**5. Add a filter and a delivery policy when the consumer needs them.** This is
the block from step 4, extended; keep one `subscription` block per consumer. The
filter is typed against every event in the list, and the dead-letter entry must
name a channel in this file.

```evt,fragment
subscription triage {
    events [ TicketRaised ]
    consumer triage_service
    when (ticketNo != "")
    delivery {
        mode at_least_once;
        maxAttempts 3;
    }
}
```

**6. Check the contract, then the project.** A single-file check proves the
payload types and internal references. A project check adds the cross-surface
wiring: an operation's emitted events must be declared, and every declared event
must be emitted by an operation.

```sh
pherolang check support.evt
pherolang check .
```

## Example

A complete contract over the support domain: one event with three payload
members, one channel, and one filtered subscriber with a delivery policy.

```evt,fixture=examples/support
import "support.mox"

event TicketRaised version "1.0.0" {
    ticketNo: String;
    title: String;
    state: TicketState;
}

channel tickets {
    publishes TicketRaised;
}

subscription triage {
    events [ TicketRaised ]
    consumer triage_service
    when (ticketNo != "")
    delivery {
        mode at_least_once;
        maxAttempts 3;
    }
}
```

A second consumer is another `subscription` block with its own event list and
filter; the event and the channel stay unchanged. If a consumer needs an event
the channel does not publish yet, add a `publishes` entry for an event declared
in the same file.

## Related reference

- [Event contracts](../reference/event-contracts.md) — every declaration form
  and validation rule.
- [Events, channels, and subscriptions](../concepts/events-channels-and-subscriptions.md)
  — how the three parts divide responsibility.
- [Services and operations](../../ddd/reference/services-and-operations.md) —
  the `emits` clause that produces an event.
- [The .evt engineering reference](../../../EVENTS.md) — the compiled model and
  the cross-surface checks.
