---
title: Applications and modules
description: Declare the application, import its domain model, and group its designs into modules.
type: reference
language: ddd
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md]
related: [docs/guide/ddd/reference/designs.md, docs/guide/ddd/reference/services-and-operations.md]
lastReviewed: 2026-11-21
---

# Applications and modules

An application declaration names the design, fixes the domain package it is
built on, and owns the application's error vocabulary. Modules group the
designs; they are labels, not boundaries.

## Syntax

```text
import "<path>.mox";

application <Name> {
    base <package>
    error <Name>(<payload>)?;          // zero or more
    module <name> {
        // services, designs, searches, children blocks
    }
}
```

A file is either a **root** (`application …`) or a **fragment** (top-level
`module` blocks), never both. A fragment is compiled through the root that
imports it.

## Semantics

- **`import` resolves relative to the declaring file.** A design next to its
  domain writes `import "support.mox";`. A fragment resolves against the
  fragment's own directory, not the root's.
- **One application owns the name, `base`, and application-scoped errors.** A
  fragment contributes modules only; its modules merge into the root in import
  order, and a fragment may reopen a module.
- **Modules group designs.** A module is a design label; it does not create a
  deployment boundary, a separate artifact, or a second namespace for
  operations. Duplicate members remain application-wide errors.
- **Errors are a declared vocabulary.** `error X(...)` at the application level
  is visible to every module. An error declared inside a module is private to
  that module. The prelude provides `NotFound` and `Conflict` implicitly.
- **`///` doc comments** directly above a `service` or `search` become its
  description.

## Constraints

- Imports precede the application (or fragment) declarations.
- `base` is the application's first member when present.
- One application per root; a file cannot hold both an application and
  top-level modules.
- Error names are application-wide unique and cannot shadow the prelude.
- A `throws` clause is explicit and complete: raising an undeclared error is
  `E0373`, and a delegation cannot narrow its target's thrown set (`E0377`).
- A module-private error referenced from another module is `E0375`.

## Examples

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support
    error TicketAlreadyResolved(String ticketNo);

    module desk {
        service TicketService {
            inject TicketRepository;
            boolean resolveTicket(String ticketNo) throws NotFound, TicketAlreadyResolved;
        }
        entity Ticket repository TicketRepository {
            findById;
            findAll;
            save;
            delete;
        }
    }

    module people {
        service PeopleService {
            inject UserRepository;
            User findUser(String userId);
        }
        entity User repository UserRepository {
            findById;
            findAll;
            save;
            delete;
        }
    }
}
```

Two modules, one application, one error vocabulary. `TicketAlreadyResolved` is
visible in both modules because it is declared on the application.

## Diagnostics

| Code | Meaning |
|---|---|
| `E0373` | An operation throws an error that is not in the visible vocabulary. |
| `E0374` | A duplicate error declaration, or one that shadows the prelude. |
| `E0375` | A module-private error referenced from outside its module. |
| `E0377` | A delegation throws a narrower set than its target. |
| `E0390`–`E0395` | Multi-file application failures: an unresolvable fragment import, a cycle, a fragment compiled as a root, and the other tree-level errors. Each is anchored at the offending import. |

## Compatibility

- A root with exactly one `.ddd` root gives the semantic graph its application
  name; a fragmented application is one application in the graph.
- A fragment is never its own design model: `pherolang ir` on a fragment is an
  error, and every host compiles the root whole.
- The application's name, modules, and error vocabulary are part of the design
  model's wire contract.
