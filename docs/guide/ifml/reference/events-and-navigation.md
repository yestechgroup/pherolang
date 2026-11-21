---
title: Events and navigation
description: The event vocabulary and its effects — navigate, refresh, action, continuations — plus the CLI command surface.
type: reference
language: ifml
section: reference
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md]
related: [docs/guide/ifml/how-to/define-navigation.md, docs/guide/ifml/reference/views-and-components.md]
lastReviewed: 2026-11-21
---

# Events and navigation

An event says what can happen on a component; its effect says where the flow
goes. The vocabulary is closed for built-in events and open for custom ones.

## Syntax

```text
on <event> (<payload>)? requires: [<Capability>, ...]? (if <expression>)? -> <effect>;

<effect> := navigate("<ViewOrContainer>", { <name>: <expression>; ... })
          | refresh("<Component>")
          | action("<Name>", { <argument>: <expression>; ... })?
          | stay;

// inside an action invocation, continuations:
on success -> <effect>;
on error   -> <effect>;
```

Built-in event words: `select`, `submit`, `click`, `change`, `load`, `save`,
`cancel`, `delete`, `confirm`, `back`. Any other word is a custom event.

A flow may also declare a terminal surface — the same interaction model in a
command vocabulary:

```text
command "<name>" {
    description: "<text>";
    requires: [<Capability>, ...];
    argument <name>: <Type>;
    option <name>: <Type> = <default>;
    prompt <name>: <kind> = <default>;
    output: <kind>;
    on run (if <expression>)? -> <effect>;
    use "<Module>" as <alias> { ... };
    command "<name>" { ... }     // subcommand, to any depth
}
```

The terminal surface, as a syntax illustration (the implementation reference
carries the full CLI example):

```ifml,fragment
command "tickets" {
    description: "Work the support ticket queue";
    requires: [ReadTicket];

    option state: TicketState = "open";

    on run -> action("ListTickets");

    command "show" {
        argument ticketNo: String;
        output: details;
        on run -> action("GetTicket");
    }
}
```
## Semantics

- **The payload names what the event provides.** `select(row)` binds the
  selected row; `submit(values)` binds the submitted form data. The payload
  name is available in the effect and in the `if` guard.
- **`requires:` is an authorization requirement** on the event, resolved like a
  view-level requirement.
- **`if` is a guard.** A false guard means the effect does not fire.
- **`navigate` carries a parameter map.** Each entry becomes a parameter of the
  target view or container; the target's `params` declares the names.
- **`refresh` re-renders a declared component in place.**
- **`action` invokes a declared action**, optionally with arguments and
  success/error continuations. The action's `target:` names the domain class it
  affects.
- **`stay` keeps the current view** — the natural error continuation.
- **A command's invocation path is its nesting.** `command "tickets"` containing
  `command "show"` runs as `tickets show`; typed arguments, options, and prompts
  bind its guards and effects like view parameters.

## Constraints

- A `navigate` target must be a declared view or container (`E0105`).
- A `refresh` target must be a declared component (`E0106`).
- An `action` reference must name a declared action (`E0104`).
- An action's `target:` must resolve to exactly one domain class (`E0109`).
- Event and view `requires:` names must be declared capabilities (`E0107`);
  `roles:` names must be declared actors (`E0108`).
- A parameter map entry must name a parameter the target declares; its
  expression is typed against the event payload and the component's `data:`
  class.
- A command's option and prompt defaults are checked structurally; generic
  identifier types and prompt names are deferred, not errors.

## Examples

```ifml,fixture=examples/support
import "support.mox";
import "support.actor";

view "Queue" {
    label "Approval queue";
    requires: [ReadTicket];

    component "details" {
        type: details;
        data: Ticket;
    }

    component "raise" {
        type: form;
        data: Ticket;
        if refundCents > 0;

        on raise(values) requires: [RaiseRefund] -> action("RaiseRefund", {
            ticket: values.ticketNo;
            on success -> refresh("details");
            on error -> stay;
        });

        on cancel -> navigate("Inbox");
    }
}

view "Inbox" {
    label "Support inbox";
    landmark: true;
}

view "Receipt" {
    params { ticket: Ticket };

    component "details" {
        type: details;
        data: Ticket;
    }
}

action "RaiseRefund" {
    target: Ticket;
}
```

Two events on one form: `raise` invokes the action and continues to the receipt
on success (or stays put on failure); `cancel` navigates back to the inbox.


## Diagnostics

| Code | Meaning |
|---|---|
| `E0104` | `action("X")` names no declared action. |
| `E0105` | `navigate("X")` names no declared view or container. |
| `E0106` | `refresh("X")` names no declared component. |
| `E0107`, `E0108` | An authorization requirement names an undeclared capability or role. |
| `E0109` | An action's `target:` is unknown or ambiguous in the domain union. |
| `W0150`–`W0154` | The `.ifml` lints (unreachable view, unused data binding, and siblings): warnings, not errors. |

## Compatibility

- The control/route manifest is derived after checking, so every navigation and
  action edge is declaration-backed by construction.
- The terminal surface is a Phero extension: it shares the event vocabulary and
  parameter bindings, and it is part of the same interaction model.
- Deferred command constructs (modality profiles, doc/completion generation, TUI
  constructs) are proposed features, never presented as implemented.
