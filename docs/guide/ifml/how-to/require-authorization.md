---
title: Require authorization
description: Gate views and events on capabilities and roles, and resolve them against an actor policy.
type: how-to
language: ifml
section: how-to
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md, docs/guide/ifml/tutorials/your-first-interaction-flow.md]
related: [docs/guide/ifml/reference/events-and-navigation.md, docs/guide/ddd/overview.md]
lastReviewed: 2026-11-21
---

# Require authorization

Use this page when a screen or an event must be limited to people holding a
capability or a role, and the limitation must be checked against the
application's actor policy.

## Problem

An authorization check written in a template or a controller is invisible to
the model: a capability can be renamed, or never declared, and the check becomes
a no-op that still compiles. You need the requirement to name a capability the
policy actually declares.

## Prerequisites

- The actor policy file (`.actor`) that declares the capabilities and actors.
- The flow imports both the domain and the policy.
- The [tutorial](../tutorials/your-first-interaction-flow.md) completed.

## Solution

**1. Import the policy next to the domain.**

```ifml,fragment
import "support.mox";
import "support.actor";
```

The host compiles the policy and supplies it to the flow's authorization pass.

**2. Require capabilities on a view.**

```ifml,fragment
view "Queue" {
    requires: [ResolveTicket];
}
```

Every name must be a declared `capability` in some provided actors block
(`E0107` naming the missing capability). Names resolve bare-name union-wide, so
the policy's module does not appear in the flow.

**3. Restrict a view to roles when the check is about who, not what.**

```ifml,fragment
roles: [Manager];
```

A role is a declared `actor` (`E0108` when it is not).

**4. Guard one event instead of the whole view.**

```ifml,fragment
on submit(values) requires: [RaiseRefund] -> action("RaiseRefund");
```

Event-level requirements combine with an `if` guard when both a capability and
a data condition apply.

**5. Check the flow.**

```sh
pherolang check app.ifml
```

The authorization pass runs whenever a policy is provided; without one it is
skipped, so a flow compiled alone cannot prove its requirements. The project
check (`pherolang check <dir>`) always gathers the imported policy.

## Example

```ifml,fixture=examples/support
import "support.mox";
import "support.actor";

view "Queue" {
    label "Manager queue";
    requires: [ResolveTicket];
    roles: [Manager];

    component "details" {
        type: details;
        data: Ticket;
    }

    component "close" {
        type: form;
        data: Ticket;

        on submit(values) requires: [ResolveTicket] -> action("CloseTicket");
    }
}

action "CloseTicket" {
    target: Ticket;
}
```

The view is limited to the `Manager` role and the `ResolveTicket` capability;
the close event re-states the capability because it is the action that actually
resolves the ticket. Both names are declared in `support.actor`.

## Related reference

- [Events and navigation](../reference/events-and-navigation.md) — where
  `requires:` and `roles:` may appear.
- [The .ifml language reference](../../../../IFML.md#authorization-references) —
  the authorization contract, including the instantiation rule.
- [Application design (.ddd)](../../ddd/overview.md) — how capabilities are
  declared and granted.
