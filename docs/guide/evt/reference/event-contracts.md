---
title: Event contracts
description: The .evt declaration forms — event payloads and versions, governance and CloudEvents declarations, channels, subscriptions, and their rules.
type: reference
language: evt
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/evt/overview.md]
related: [docs/guide/evt/concepts/events-channels-and-subscriptions.md, docs/guide/evt/how-to/declare-an-event-contract.md]
lastReviewed: 2026-11-21
---

# Event contracts

This page is the authority for the declarations a `.evt` file may contain and
for the rules each declaration must satisfy.

## Syntax

```evt,fragment
import "<domain>.mox"

event <Name> version "<version>" {
    <payloadName>: <Type>;
    owner <name>;
    classification <word>;
    retention "<period>";
    pii [ <payloadName> ... ];
    cloudEvents {
        type "<value>"
        source "<value>"
        subject "<value>"
    }
}

channel <name> of <scope> {
    publishes <Event>;
}

subscription <name> {
    events [ <Event> ... ]
    consumer <name> scope <scope>
    when (<expression>)
    delivery {
        mode <mode>;
        maxAttempts <count>;
        backoffSeconds <seconds>;
        timeoutSeconds <seconds>;
        deadLetter <channel>;
    }
}
```

The `version` clause, a channel's `of` scope, and an event's governance and
`cloudEvents` blocks are optional; so are a subscription's `scope`, `when`, and
`delivery` clauses. In an event body the members may be written in any order;
in a subscription the clause order is fixed: `events`, `consumer`, `scope`,
`when`, `delivery`.

## Semantics

- **Imports come first.** One or more `import "<path>"` lines precede the first
  declaration. Each path names a `.mox` domain model, resolved relative to the
  contract's own directory. An import line has no trailing semicolon.
- **Three declaration kinds.** `event`, `channel`, and `subscription` are the
  only top-level declarations. They may be interleaved in the file.
- **Event payloads.** Each `<payloadName>: <Type>;` line declares one payload
  member. A type is a domain kind — class, enum, datatype, interface, or
  vocabulary — or one of the shared primitives such as `String`, `int`, or
  `boolean`. A payload member holds one value of its type; the declaration has
  no multiplicity syntax.
- **Version.** `version "<version>"` is free-form text carried exactly as
  written. The examples use the familiar `X.Y.Z` shape, but no spelling is
  required and the text is never compared, ordered, or validated. The producer
  and its consumers decide what a version change means.
- **Governance declarations.** `owner`, `classification`, `retention`, and `pii`
  are labels stored as written. The language does not interpret an owner name, a
  classification word, or a retention period. The `pii` list names payload
  members of the same event that carry personal data.
- **CloudEvents declarations.** The `cloudEvents` block declares three
  attributes of a CloudEvents message: its type, its source, and an optional
  subject. CloudEvents 1.0 is an industry standard; this block is a Phero Lang
  implementation that records the attributes in the contract. When the block is
  present its values are authoritative for a generator; when it is absent the
  generator derives the attributes itself.
- **Channels.** A channel lists the events published on one logical stream.
  `publishes <Event>;` is membership and consumption-side wiring, not proof that
  anything produces the event.
- **Production and consumption.** A `.ddd` service operation's `emits` clause and
  a `.dmn` decision's `emits` block name the events a piece of work produces. A
  project check requires an operation's emitted event to be declared, and
  requires every declared event to be emitted by an operation. A decision's
  emitted names are checked against the contract the project provides.
- **Scope.** A channel's `of <scope>` clause and a subscription's
  `scope <name>` clause are opaque labels. They are carried as written and never
  resolved or compared. A transport uses them to parameterize a topic family.
- **Subscriptions.** A subscription names exactly one consumer, the events it
  receives, an optional scope binding, an optional filter, and an optional
  delivery policy. The same event may appear in any number of subscriptions.
- **Filters.** The interior of `when (...)` is an expression, stored as written,
  that must type as `boolean` against every event in the subscription's list.
  The condition decides per arriving event whether the consumer receives it.
- **Delivery.** `mode` is required once the `delivery` block is present and is
  one of `at_most_once`, `at_least_once`, or `effectively_once`. The three tuning
  entries are positive integers. `deadLetter` names a channel declared in the
  same file. The block declares a policy; it does not execute delivery.

## Constraints

| Rule | Consequence when broken |
|---|---|
| An `import` names a provided domain model | The import cannot be resolved. |
| Every payload type is a primitive or a resolvable domain type | The type is unknown. |
| Event, channel, and subscription names are unique within the file | A duplicate declaration is an error. |
| Payload member names are unique within their event | A duplicate member is an error. |
| A `publishes` entry names an event in the same file | The channel names an unknown event. |
| A subscription's `events` list names events in the same file | The subscription names an unknown event. |
| A subscription's `events` list is not empty | The subscription would deliver nothing. |
| A subscription names a non-empty consumer | The subscription would deliver nowhere. |
| A `pii` entry names a payload member of the same event | The list names something the event does not carry. |
| A `cloudEvents` type and source are non-empty | The declared attributes are unusable. |
| A delivery mode is one of the three vocabulary words | The mode is unknown. |
| Delivery attempt, backoff, and timeout values are positive | The setting is invalid. |
| A `deadLetter` entry names a channel in the same file | The policy names an unknown channel. |
| A filter parses and types as `boolean` against every subscribed event | The filter cannot decide the route. |

An error anywhere in the contract, or in an imported domain model, means no
usable compiled result. A warning does not prevent compilation.

## Examples

A contract that uses every declaration form: payloads typed by the support
domain, governance labels, CloudEvents attributes, a scoped channel, and a
subscription with a filter and a delivery policy.

```evt,fixture=examples/support
import "support.mox"

event TicketRaised version "1.0.0" {
    ticketNo: String;
    title: String;
    state: TicketState;
    owner support_team;
    classification internal;
    retention "7y";
    pii [ ticketNo ];
    cloudEvents {
        type "nz.example.support.ticketraised.v1"
        source "/support/tickets"
        subject "{ticketNo}"
    }
}

event TicketResolved version "1.0.0" {
    ticketNo: String;
    state: TicketState;
}

channel tickets of account {
    publishes TicketRaised;
    publishes TicketResolved;
}

subscription refundDesk {
    events [ TicketRaised TicketResolved ]
    consumer refund_approval_service scope account
    when (ticketNo != "")
    delivery {
        mode at_least_once;
        maxAttempts 5;
        backoffSeconds 10;
        timeoutSeconds 30;
        deadLetter tickets;
    }
}
```

`TicketRaised` carries governance labels and declared CloudEvents attributes;
`TicketResolved` carries only payload members and a version. Both are published
on `tickets`, whose `account` scope matches the scope `refundDesk` binds. The
filter reads `ticketNo`, which both subscribed events carry, and the dead-letter
entry points at the declared `tickets` channel.

## Diagnostics

| Finding | Cause |
|---|---|
| An imported file was not provided | An `import` path names no domain model the check can read. |
| Unknown type | A payload type is neither a primitive nor a declared domain type. |
| Unknown event on a channel | A `publishes` entry names no event in the file. |
| Unknown event in a subscription | An `events` list entry names no event in the file. |
| Duplicate event, channel, or subscription | Two declarations of one kind share a name. |
| Duplicate payload member | Two members of one event share a name. |
| A subscription subscribes to no events | Its `events` list is empty. |
| A subscription names an empty consumer | Its `consumer` clause has no name. |
| Empty CloudEvents attribute | A declared `type` or `source` is empty. |
| Unknown payload member in `pii` | A `pii` entry names a member the event does not carry. |
| Unknown delivery mode | The mode is not one of the three vocabulary words. |
| Non-positive delivery setting | An attempt, backoff, or timeout value is zero or negative. |
| Unknown dead-letter channel | The `deadLetter` entry names no channel in the file. |
| Invalid filter | The `when` text does not parse, or does not type as `boolean` against every subscribed event. |

A project check adds three wiring findings across the surfaces:

| Code | Finding |
|---|---|
| `X0003` | A service operation emits an event no contract declares. |
| `X0004` | A declared event is emitted by no service operation. |
| `X0007` | A subscription's consumer resolves to no declared deployment component. |

The first two fire only when both an application design and a contract are part
of the checked application; the third fires when a deployment model is present.

## Compatibility

- An event's `version` is free-form text with no ordering or comparison. Two
  versions of one event share a name; the version label is how a consumer
  distinguishes generations, and its meaning is a producer decision.
- The compiled contract keeps every name, scope label, governance label, and
  CloudEvents value exactly as written. Consumers resolve channel membership and
  subscription lists against the declaring file.
- Channel and subscription scope labels are never validated, so a consumer must
  treat them as opaque identifiers.
- A contract that compiles contains only declarations. Delivery, transport,
  broker configuration, and consumer addresses are outside the contract; a
  deployment model and the consumer's own runtime provide them.
- When an event declares CloudEvents attributes, those values take precedence
  over any derived mapping a generator would otherwise use.
