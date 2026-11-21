---
title: Define navigation
description: Connect views and containers with checked navigation flows — parameter maps, guards, wizards, and refresh events.
type: how-to
language: ifml
section: how-to
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md, docs/guide/ifml/tutorials/your-first-interaction-flow.md]
related: [docs/guide/ifml/reference/events-and-navigation.md]
lastReviewed: 2026-11-21
---

# Define navigation

Use this page when a screen must lead to another screen, conditionally or with
data carried along, and the target needs to be checked rather than remembered.

## Problem

Navigation is usually scattered across templates and router configuration.
Nothing stops a button from pointing at a screen that no longer exists, or
passing a parameter the target never declares. You need the moves to be part of
the model, so a broken link is a compile error.

## Prerequisites

- A flow with at least two declared views (or a view with containers).
- The domain imported if the flow binds data.
- The [tutorial](../tutorials/your-first-interaction-flow.md) completed.

## Solution

**1. Declare the target.** A navigation target is a declared `view` or a
`container`. It is never a free string.

```ifml,fragment
view "Queue" {
    params { ticket: Ticket };
}
```

**2. Attach an event with `navigate` and a parameter map.**

```ifml,fragment
on select(row) -> navigate("Queue", {
    ticket: row.ticketNo
});
```

`row` is the selected list/table row; `row.ticketNo` is checked against the
component's `data:` class. Parameter names on the right must exist in the
target's `params` block.

**3. Guard the move when it is conditional.**

```ifml,fragment
on approve(row) if agent.active -> action("ApproveRefund");
```

**4. Group steps as containers.** A view can declare `xor: true` and nest
containers; exactly one is shown at a time, and `default: true` names the entry.

```ifml,fragment
container "Step2" {
    component "address" {
        type: form;
        data: Ticket;
        on submit(values) -> navigate("Step3");
    }
}
```

**5. Use `refresh` to re-render a component in place.**

```ifml,fragment
on save(values) -> action("RaiseRefund", {
    ticket: values.ticketNo;
    on success -> refresh("details");
});
```

**6. Check the flow.**

```sh
pherolang check app.ifml
```

A target that is not a declared view or container (`E0105`), a missing action
(`E0104`), or a refresh target that is not a component (`E0106`) is reported at
the reference.

## Example

A two-step triage view: select a ticket, then decide on it.

```ifml,fixture=examples/support
import "support.mox";

view "Triage" {
    xor: true;

    container "Select" {
        default: true;

        component "tickets" {
            type: list;
            data: Ticket;
            fields: [ticketNo, title, refundCents];

            on select(row) -> navigate("Decide", {
                ticket: row.ticketNo
            });
        }
    }

    container "Decide" {
        component "details" {
            type: details;
            data: Ticket;
        }

        component "approve" {
            type: form;
            data: Ticket;
            on approve(values) -> action("ApproveRefund");
        }
    }
}

action "ApproveRefund" {
    target: Ticket;
}
```

`Triage` is one view with two containers: `Select` is the default, and
selecting a row navigates to the `Decide` container with the ticket number. Both
container targets are declared in the same view.

## Related reference

- [Events and navigation](../reference/events-and-navigation.md) — the event
  vocabulary, parameter maps, and continuations.
- [Views and components](../reference/views-and-components.md) — containers,
  component types, and `data:`.
- [The .ifml language reference](../../../../IFML.md#syntax-examples) — more
  worked flows.
