---
title: Modules and imports
description: Load domain models, actor policies, decision models, and reusable interaction modules — and instantiate a pattern with use.
type: reference
language: ifml
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md]
related: [docs/guide/ifml/reference/views-and-components.md]
lastReviewed: 2026-11-21
---

# Modules and imports

A flow compiles as one closure: the entry file plus every file it imports. The
imports supply the domain, the policies, the decisions, and the reusable
interaction modules the entry file composes.

## Syntax

```text
import "<path>.mox";      // the domain the flow binds to
import "<path>.actor";    // capabilities and roles for requires:/roles:
import "<path>.dmn";      // decision services for data: bindings
import "<path>.ifml";     // a pattern module

module "<Name>" {          // a reusable interaction module
    input { name: Type = default, ... }
    output { name: Type, ... }
    // containers, components, events, use
}

use "<Module>" as <alias> { <input>: <binding>; ... };
```

A module and its instantiation:

```ifml,fragment
module "Pager" {
    input { page: Int = 1, pageSize: Int = 25 }
    output { page: Int }

    component "pager" {
        type: list;
        data: Item;
        fields: [name];
    }
}

use "Pager" as pager { page: 2; };
```

The module declares an input named `page`; the `use` binds it. The module's
component binds the generic `Item` type, which the consuming app maps to its own
class where it has one.

## Semantics

- **Imports resolve relative to the importing file.** A flow next to its domain
  writes `import "support.mox";`. An imported pattern module resolves its own
  imports relative to itself.
- **A module is a reusable view fragment, not a separate compilation.** Its
  inputs are bound at each `use` site; its outputs are what the instantiation
  exposes to the consuming view. A module may `use` other modules.
- **Only instantiated modules are checked as part of the app.** A module the
  entry file `use`s contributes its views, components, and references to the
  app's surface; sibling modules the app never instantiates stay inert, so the
  reference pattern library can carry app-facing names it does not own.
- **Imports are keyed by `(importing file, path as written)`.** The same module
  may be imported under different relative paths by different files; they are
  distinct entries until resolved.
- **Declaration order.** Every `import` precedes the declarations of the file.

## Constraints

- An import that cannot be read is an error naming the resolved path.
- A `use` site must name a module the compilation provides; its input bindings
  must match the module's declared inputs.
- A module instantiated with a missing required input, or an unknown input, is
  an error.
- A module's `output` names are available to the consuming view; other module
  internals are not.
- The pattern library lives in `patterns/ifml/`; its modules are domain-agnostic
  and bind through their inputs (often a generic `Item` type).

## Examples

A self-contained flow over the support domain, importing the domain and the
actor policy:

```ifml,fixture=examples/support
import "support.mox";
import "support.actor";

view "Inbox" {
    label "Support inbox";
    landmark: true;
    requires: [ReadTicket];

    component "tickets" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title];

        on select(row) -> navigate("Detail", {
            ticket: row.ticketNo
        });
    }
}

view "Detail" {
    params { ticket: Ticket };

    component "details" {
        type: details;
        data: Ticket;
    }
}
```

## Diagnostics

| Code | Meaning |
|---|---|
| `E0004` | Two imported files declare the same module name. |
| `E0104` | An `action("X")` reference names no declared action. |
| `E0105` | A `navigate("X")` target is not a declared view or container. |
| `E0106` | A `refresh("X")` target is not a declared component. |
| `E0109` | An action's `target:` names no class, or several classes, in the domain union. |

## Compatibility

- The model keeps `use` references; a consumer expands them when it needs the
  resolved module contents, and the compact artifact stays the default.
- Import identities are the host's keys; moving a file changes the key, not the
  module's name.
- The pattern library's wire shape is pinned by committed goldens, so a module
  change is a reviewable model diff.
