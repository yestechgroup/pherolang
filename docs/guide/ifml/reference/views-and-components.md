---
title: Views and components
description: The view, container, and component declarations — parameters, flags, component types, data bindings, and the properties the checker understands.
type: reference
language: ifml
section: reference
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md]
related: [docs/guide/ifml/reference/events-and-navigation.md, docs/guide/ifml/concepts/views-and-navigation.md]
lastReviewed: 2026-11-21
---

# Views and components

A view is a screen; a container groups part of a screen; a component is the
element that shows or collects data. This page is the authority for their
declarations.

## Syntax

```text
view "<Name>" {
    params { <name>: <Type>, ... };
    label "<text>";
    landmark: true;          // an entry view
    xor: true;               // show exactly one child container
    modal: true;
    requires: [Capability, ...];
    roles: [Role, ...];

    container "<Name>" {
        default: true;       // the entry container of an xor group
        // components, nested containers, events
    }

    component "<Name>" {
        type: <component-type>;
        data: <Class>;
        fields: [feature, ...];
        column "<Label>" -> expr <expression>;
        filter: <expression>;
        if <expression>;
        mode: edit;
        // events
    }
}

component "<Name>" { ... }   // a top-level component used by modules
container "<Name>" { ... }
```

Component types are `list`, `form`, `details`, `search`, `tree`, `chart`,
`table`, `button`, `link`, `menu`, `image`, `embedded`, or a custom type name.

## Semantics

- **`params` are the view's inputs.** A navigation flow's parameter map must
  name them; their types are domain classes, primitives, or named types.
- **`data:` binds the component's self type.** `fields`, `column`, `filter`,
  `if`, and event expressions are typed against the named class.
- **`landmark: true`** marks an entry view — the flow's entry points are the
  landmarks, not an arbitrary first declaration.
- **`xor: true`** on a view (or a container's siblings) forms an exclusive
  group: exactly one child shows, and `default: true` names the entry.
- **`modal: true`** marks a view presented over another.
- **`requires:`/`roles:`** are view-level authorization requirements; see
  [Require authorization](../how-to/require-authorization.md).
- **`mode: edit`** selects the form's editing mode; forms default to create.
- **Nesting.** Containers may nest to any depth; components and events live in
  views and containers.

## Constraints

- A component's `data:` must resolve to exactly one domain class
  (`E0100` when it resolves to none; a domain ambiguity is never rescued by the
  decision dimension).
- `fields` and `column` expressions must name features of the bound class
  (`E0102` for an unknown feature or operation on a navigation path).
- A navigation target must be a declared view or container (`E0105`); a refresh
  target must be a declared component (`E0106`).
- An action's `target:` must resolve to exactly one domain class (`E0109`).
- Authorization names must resolve against a provided actor policy (`E0107`,
  `E0108`).
- A view or container name is unique in the compilation; a component name is
  unique where it is declared.

## Examples

```ifml,fixture=examples/support
import "support.mox";

view "TicketBoard" {
    label "Ticket board";

    container "Open" {
        default: true;

        component "openTickets" {
            type: table;
            data: Ticket;
            fields: [ticketNo, title];
            filter: refundCents > 0;
            column "Refund" -> expr refundCents / 100;
        }
    }

    container "Detail" {
        component "details" {
            type: details;
            data: Ticket;
        }

        component "raise" {
            type: form;
            data: Ticket;
            mode: edit;
            if refundCents > 0;
        }
    }
}
```

A view with two containers: the open tickets table (default) and a detail
container with a details panel and an editing form.

## Diagnostics

| Code | Meaning |
|---|---|
| `E0100` | `data:` names no domain class (or is ambiguous across packages). |
| `E0102` | An expression names a feature or operation the bound class does not have. |
| `E0104`, `E0105`, `E0106` | An action, navigation, or refresh reference is undeclared. |
| `E0107`, `E0108` | An authorization capability or role is not declared. |
| `E0109` | An action target is unknown or ambiguous in the domain union. |

## Compatibility

- The interaction model is a versioned artifact: views, containers, components,
  properties, and events are part of its wire contract.
- `data:` resolution is domain-only in the model; a decision-service binding is
  a check-only fact and keeps the bare spelling.
- Unknown top-level properties are carried by the model; the checker consumes
  the ones it documents (`type`, `data`, `fields`, `column`, `filter`, `if`,
  `mode`, authorization lists).
