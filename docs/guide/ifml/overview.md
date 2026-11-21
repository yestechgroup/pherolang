---
title: Interaction flows (.ifml)
description: Describe how people move through an application — views, components, events, navigation, actions, and the data each screen shows.
type: overview
language: ifml
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/ifml/concepts/omg-ifml-conformance.md, docs/guide/ifml/reference/views-and-components.md]
lastReviewed: 2026-11-21
---

# Interaction flows (.ifml)

An `.ifml` file describes the interaction model of an application: the views
people see, the components inside them, the events they trigger, where those
events navigate, and which data each screen shows.

## What is it?

A domain model says what the application knows; an application design says how
it is built; an interaction flow says how people move through it. It is a
text-first model in the spirit of OMG IFML 1.0: views and view containers form
the screen structure, components such as lists, tables, details, and forms show
and collect data, and events connect them.

The flow binds to the same domain model the rest of the application uses. A
component's `data:` names a domain class, its expressions use that class's
features, and a form's values are typed against it. When the flow compiles, the
data bindings are checked — an unknown class or feature is an error, not a
runtime surprise.

## What problems does it solve?

Screen structure and navigation are usually the least reviewable part of an
application: they live in templates, router configuration, and event handlers,
and nothing checks that a link points at a screen that exists or that a filter
names a real feature. A rename breaks the UI silently.

An interaction flow makes those facts explicit and checkable. Navigation targets
must be declared views or containers; action invocations must name declared
actions; authorization references must resolve against an actor policy; and data
bindings are typed against the domain. The route structure is then derived from
the model rather than maintained by hand.

## Key capabilities

- **Views and containers.** Declare screens, parameters, landmarks, and
  container groups for wizards and conditional navigation.
- **Components.** Lists, tables, details, forms, charts, and custom component
  sites with typed data bindings.
- **Events and navigation.** `navigate`, `refresh`, and `action` handlers with
  parameter maps, guards, and success/error continuations.
- **Data binding.** `data:` against domain classes, `fields`, `column`
  expressions, `filter` predicates, and row-scoped parameter expressions.
- **Authorization references.** `requires:` and `roles:` on views and events,
  resolved against an actor policy.
- **Decision data.** A component may show a decision service's outputs when a
  decision model is provided.
- **CLI commands.** The same interaction model expressed as terminal commands.
- **Composable patterns.** Reusable interaction modules composed with `use`.

## A simple example

An inbox that navigates to a queue, where a form raises a refund request:

```ifml,fixture=examples/support
import "support.mox";

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

`Inbox` lists the tickets with a positive refund amount; selecting one
navigates to `Queue` with the ticket's number. `Queue` shows the ticket and a
form that invokes the declared `RaiseRefund` action.

## How it fits into Phero Lang

An interaction flow imports the domain model it binds to. Around it:

- `.ddd` declares the application services; an `action` in a flow names the
  action the application exposes, and the two are checked together when both
  are present.
- `.actor` declares the capabilities and actors that `requires:` and `roles:`
  resolve against.
- `.dmn` decision models provide a decision service a component can bind to.
- `.deploy` projects the application and its flows onto runtime targets.
- `.uc` use case patterns relate actors and system behavior across the
  application.

The flow is the interaction dimension of the same model, not a separate
description of the system.

## Getting started

- [Tutorial — your first interaction
  flow](../tutorials/your-first-interaction-flow.md) builds the example above
  step by step.
- [Define navigation](../how-to/define-navigation.md) covers views, containers,
  guards, and parameter maps.
- [Bind domain data](../how-to/bind-domain-data.md) covers `data:`, fields,
  expressions, and filters.
- [Require authorization](../how-to/require-authorization.md) wires `requires:`
  and `roles:` to an actor policy.

## Further reading

- [Views and navigation](../concepts/views-and-navigation.md) explains the
  interaction model.
- [OMG IFML conformance](concepts/omg-ifml-conformance.md) states exactly how
  the language relates to the standard.
- [Views and components](../reference/views-and-components.md) and [events and
  navigation](../reference/events-and-navigation.md) are the reference pages.
- [The .ifml language reference](../../../IFML.md) is the complete engineering
  reference, including the model contract and the pattern library.
