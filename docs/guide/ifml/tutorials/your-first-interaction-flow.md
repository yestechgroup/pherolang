---
title: Tutorial — your first interaction flow
description: "Build a two-screen support flow over an existing domain: a list, navigation with parameters, a form, an action, and an authorization reference."
type: tutorial
language: ifml
section: tutorials
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md, docs/guide/mox/overview.md]
related: [docs/guide/ifml/how-to/define-navigation.md, docs/guide/ifml/reference/views-and-components.md]
lastReviewed: 2026-11-21
---

# Tutorial — your first interaction flow

This tutorial builds an interaction flow over the support-desk domain: a ticket
inbox, a queue screen reached by navigating from a selected row, a form that
raises a refund, and an authorization reference checked against the shipped
actor policy.

## What you'll learn

- Declare a view with a list component bound to a domain class.
- Navigate between views with a parameter map.
- Bind filters and fields to domain features.
- Add a form that invokes a declared action.
- Gate a view on a capability from an actor policy.
- Check the flow and read its diagnostics.

## Prerequisites

- Phero Lang installed, with `pherolang --version` working.
- The repository checked out. This tutorial uses
  `examples/support/support.mox` and `examples/support/support.actor`.
- The [interaction flow overview](../overview.md) read once.

## What we're building

One file, `app.ifml`, next to a copy of the support domain and its policy. When
it is done, `pherolang check app.ifml` prints `OK app.ifml`.

Create a working directory and copy the two files:

```sh
mkdir support-flow
cd support-flow
cp <path-to-repository>/examples/support/support.mox .
cp <path-to-repository>/examples/support/support.actor .
```

## Step 1 — Declare the first view

```ifml,fixture=examples/support
import "support.mox";

view "Inbox" {
    label "Support inbox";
    landmark: true;

    component "tickets" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title];
    }
}
```

`import` resolves relative to the flow file, so `support.mox` must sit next to
`app.ifml`. `Inbox` is a view; `tickets` is a list component whose `data:` names
the domain class `Ticket`, so `ticketNo` and `title` are checked as real
features. `landmark: true` makes it an entry view.

```sh
pherolang check app.ifml
```

You should see `OK app.ifml`.

## Step 2 — Navigate to a second view

```ifml,fixture=examples/support
import "support.mox";

view "Inbox" {
    label "Support inbox";
    landmark: true;

    component "tickets" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title];

        on select(row) -> navigate("Queue", {
            ticket: row.ticketNo
        });
    }
}

view "Queue" {
    params { ticket: Ticket };
    label "Approval queue";
}
```

The event `select(row)` navigates to `Queue` and passes `row.ticketNo` as the
`ticket` parameter. The target must be a declared view or container, and the
parameter name must exist on the target's `params` block.

```sh
pherolang check app.ifml
```

## Step 3 — Bind the data

```ifml,fixture=examples/support
import "support.mox";

view "Inbox" {
    label "Support inbox";
    landmark: true;

    component "tickets" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title, refundCents];
        filter: refundCents > 0;

        on select(row) -> navigate("Queue", {
            ticket: row.ticketNo
        });
    }
}

view "Queue" {
    params { ticket: Ticket };
    label "Approval queue";

    component "details" {
        type: details;
        data: Ticket;
    }

    component "raise" {
        type: form;
        data: Ticket;
        if refundCents > 0;

        on raise(values) -> action("RaiseRefund");
    }
}

action "RaiseRefund" {
    target: Ticket;
}
```

`filter:` keeps only the tickets with a refund pending. The queue shows the
ticket, and the form is visible only while the amount is positive. `action("…")`
must resolve to a declared `action`, which names the domain class it affects.

```sh
pherolang check app.ifml
```

## Step 4 — Require the capability

```ifml,fixture=examples/support
import "support.mox";
import "support.actor";

view "Inbox" {
    label "Support inbox";
    landmark: true;

    component "tickets" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title, refundCents];
        filter: refundCents > 0;

        on select(row) -> navigate("Queue", {
            ticket: row.ticketNo
        });
    }
}

view "Queue" {
    params { ticket: Ticket };
    label "Approval queue";
    requires: [RaiseRefund];

    component "details" {
        type: details;
        data: Ticket;
    }

    component "raise" {
        type: form;
        data: Ticket;
        if refundCents > 0;

        on raise(values) requires: [RaiseRefund] -> action("RaiseRefund");
    }
}

action "RaiseRefund" {
    target: Ticket;
}
```

The actor policy declares `capability RaiseRefund on Ticket`, so the view
requirement and the event requirement both resolve. Without the policy import,
the authorization pass has nothing to check against.

```sh
pherolang check app.ifml
```

## Step 5 — Check the finished flow

The complete example below is the file produced by the steps. Check it:

```sh
pherolang check app.ifml
```

## Complete example

```ifml,fixture=examples/support
import "support.mox";
import "support.actor";

view "Inbox" {
    params { tickets: Ticket };
    label "Support inbox";
    landmark: true;

    component "tickets" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title, refundCents];
        filter: refundCents > 0;

        on select(row) -> navigate("Queue", {
            ticket: row.ticketNo
        });
    }
}

view "Queue" {
    params { ticket: Ticket };
    label "Approval queue";
    requires: [RaiseRefund];

    component "details" {
        type: details;
        data: Ticket;
    }

    component "raise" {
        type: form;
        data: Ticket;
        if refundCents > 0;

        on raise(values) requires: [RaiseRefund] -> action("RaiseRefund");
    }
}

action "RaiseRefund" {
    target: Ticket;
}
```

## Expected result

```sh
$ pherolang check app.ifml
OK app.ifml
```

The command exits with status 0. The flow declares two views, one action, and
two authorization references; every navigation target, parameter, data binding,
and capability resolves.

## Next steps

- [Define navigation](../how-to/define-navigation.md) adds containers, guards,
  and refresh handlers.
- [Bind domain data](../how-to/bind-domain-data.md) covers the expression
  surface in detail.
- [Require authorization](../how-to/require-authorization.md) explains how the
  actor tables are gathered and checked.
- The [support-desk flow](../../../../examples/support/support.ifml) is the full
  example, including pattern composition and decision data.
