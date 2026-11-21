---
title: Views and navigation
description: The interaction model behind .ifml — screens, components, events, flows, and the data each one binds.
type: concept
language: ifml
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md]
related: [docs/guide/ifml/reference/views-and-components.md, docs/guide/ifml/reference/events-and-navigation.md]
lastReviewed: 2026-11-21
---

# Views and navigation

An interaction flow describes the surface of an application as a set of
declarations: what the user sees, what they can do, and where each action leads.
Nothing about the surface is implicit in a template or a router.

## Overview

The model has five kinds of declaration:

- **View** — one screen. It has optional parameters, a label, a landmark flag
  that marks it as an entry point, and a body of components or containers.
- **Container** — a named section inside a view, used for wizards and
  conditional sections. A view with `xor: true` shows exactly one container at
  a time.
- **Component** — a UI element inside a view or container: a list, a table, a
  detail panel, a form, a chart, or a custom site. It binds to a domain class
  with `data:`.
- **Event** — a trigger on a component (`select`, `submit`, `save`, `edit`,
  `run`, …) with an optional payload, guard, and authorization requirement.
- **Effect** — what the event does: `navigate` to a view or container with a
  parameter map, `refresh` a component, or invoke a declared `action`.

Actions are declared separately, with the domain class they affect. That
declaration is the join point between the interaction model and the rest of the
application: the application's service operations provide the behavior, and the
flow names the action a control invokes.

## Why does it matter?

The interaction model is the part of an application most likely to drift: a
route changes, a filter names a renamed feature, a button points at a deleted
screen. When those facts live in templates, only a click-through finds them.

Making them declarations means a rename breaks the build instead of the UI, and
the navigation graph can be derived, reviewed, and tested. It also means the
same interaction model can feed different renderers — a web client, a terminal,
or a generator — without each re-deciding what the screen says.

## How it works

A flow is checked in stages:

1. **Resolution** — imports load the domain, the actor policy, and any pattern
   modules; views, containers, components, and actions are indexed by name.
2. **Interaction references** — every `navigate`, `refresh`, and `action` target
   must be declared, and an action's `target:` must resolve to a domain class.
3. **Typed bindings** — `data:` must resolve to a class, and the expressions in
   `fields`, `filter`, `column`, guards, and parameter maps are typed against
   it.
4. **Authorization** — when a policy is provided, every `requires:` name must
   be a declared capability and every `roles:` name a declared actor.
5. **Decision data** — when decision models are provided, a component's `data:`
   may name a decision service after the domain classes fail to match.

A stage that has nothing to check against is skipped rather than guessed, so a
flow compiled alone verifies less than a flow compiled with its policy.

## Example

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

Two views, two components in the second, one navigation, one action. Every name
in the flow is declared somewhere in the compilation or the imported domain.

## Relationship to other concepts

- The **domain model** ([the .mox overview](../mox/overview.md)) supplies the
  classes and features every binding resolves against.
- An **application design** ([.ddd](../ddd/overview.md)) declares the services
  and capabilities behind the actions and requirements.
- An **actor policy** ([.actor](../../../../docs/LANGUAGE.md#actor-policy-files-actor))
  declares the capabilities `requires:` and the actors `roles:` resolve to.
- A **deployment model** ([.deploy](../../../DEPLOY.md)) binds an application
  and its flows to runtime targets.

## Further reading

- [Views and components](../reference/views-and-components.md) — the exact
  declaration syntax.
- [Events and navigation](../reference/events-and-navigation.md) — the event
  vocabulary and effects.
- [OMG IFML conformance](omg-ifml-conformance.md) — how this model relates to
  the standard.
