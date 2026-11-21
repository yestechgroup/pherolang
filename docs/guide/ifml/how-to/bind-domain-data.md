---
title: Bind domain data
description: Bind a component to a domain class and use its features in fields, columns, filters, guards, and parameter expressions.
type: how-to
language: ifml
section: how-to
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md, docs/guide/ifml/tutorials/your-first-interaction-flow.md]
related: [docs/guide/ifml/reference/views-and-components.md]
lastReviewed: 2026-11-21
---

# Bind domain data

Use this page when a screen must show domain data — a list of records, a
record's details, a table with computed columns — and the bindings must be
checked against the model.

## Problem

A template that names `refundCents` breaks silently when the feature is renamed.
A filter written as a string is never checked. You need the screen's data
bindings to be part of the model, typed against the class they name.

## Prerequisites

- The domain model imported by the flow.
- A view with a component.
- The [tutorial](../tutorials/your-first-interaction-flow.md) completed.

## Solution

**1. Bind the component to a class with `data:`.**

```ifml,fragment
component "tickets" {
    type: list;
    data: Ticket;
}
```

The name must resolve to exactly one class in the imported domain (`E0100`
when it resolves to none, and the ambiguity error when several packages declare
it).

**2. Select the fields to show.**

```ifml,fragment
fields: [ticketNo, title, refundCents];
```

Each name is checked as a feature of the `data:` class.

**3. Compute a column when the display differs from the stored value.**

```ifml,fragment
column "Refund" -> expr refundCents / 100;
```

**4. Filter the rows.**

```ifml,fragment
filter: refundCents > 0;
```

The predicate uses the same expression language as the rest of Phero Lang.

**5. Guard a component or an event.**

```ifml,fragment
component "raise" {
    type: form;
    data: Ticket;
    if refundCents > 0;
}
```

**6. Pass row values as parameters.**

```ifml,fragment
on select(row) -> navigate("TicketDetail", {
    ticket: row.ticketNo
});
```

**7. Check the flow.**

```sh
pherolang check app.ifml
```

## Example

```ifml,fixture=examples/support
import "support.mox";

view "Ledger" {
    label "Refund ledger";

    component "tickets" {
        type: table;
        data: Ticket;
        fields: [ticketNo, title, refundCents];
        filter: refundCents > 0;
        column "Refund" -> expr refundCents / 100;

        on select(row) -> navigate("TicketDetail", {
            ticket: row.ticketNo
        });
    }
}

view "TicketDetail" {
    params { ticket: Ticket };

    component "details" {
        type: details;
        data: Ticket;
    }
}
```

The table shows only tickets with a pending refund, computes the refund column
from cents, and opens the selected ticket's detail view.

A binding that names no class is refused:

```ifml,invalid=E0100,fixture=examples/support
import "support.mox";

view "Home" {
    component "list" {
        type: list;
        data: Unknown;
    }
}
```

## Related reference

- [Views and components](../reference/views-and-components.md) — `data:`,
  `fields`, `column`, `filter`, and component types.
- [Events and navigation](../reference/events-and-navigation.md) — row-scoped
  parameters and continuations.
- [The .ifml language reference](../../../../IFML.md#typed-bindings) — the typed
  binding contract and its documented deferrals.
