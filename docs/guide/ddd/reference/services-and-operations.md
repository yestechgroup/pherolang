---
title: Services and operations
description: Declare an application service, inject repositories, write operation signatures and delegations, and use the checked statement body.
type: reference
language: ddd
section: reference
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ddd/overview.md, docs/guide/ddd/reference/designs.md]
related: [docs/guide/ddd/how-to/write-an-operation-body.md, docs/guide/ddd/reference/state-machines.md]
lastReviewed: 2026-11-21
---

# Services and operations

A service is the module's public surface: it injects the repositories and
decision services its operations need, and declares the operations that
implement use cases.

## Syntax

```text
service <Name> {
    inject <Repository>;                  // one per line
    inject decision <qualified-name>;

    <Type> <name>(<params>) <clauses>;    // declared
    <name> => <Target>.<operation> <clauses>;   // delegated
    protected <either form>;

    <Type>? <name>(<params>) <clauses> => <Repository> {
        // statement body
    }
}
```

`<clauses>` appear in one canonical order:

```text
capability <a, b>?  invokes <decision.service, …>?  emits <A, B>?
transition <Trigger>?  throws <Error, …>?  when (<condition>)?
```

## Semantics

- **`inject` binds a dependency by its declared name.** A body's repository
  receiver (`=> TicketRepository`) binds as `self`; every other injection is
  available by its name.
- **Declared operations declare their own signature.** The return type is
  required for a declared operation; the parameters and return type are typed
  against the imported domain.
- **Delegated operations declare no signature.** `save => TicketRepository.save`
  copies the callee's signature. The delegation target must be in the same
  module.
- **`throws` is the error vocabulary.** The application and module error
  declarations plus the `NotFound`/`Conflict` prelude. A body raise outside it
  is an error, never inferred.
- **`when` is a boolean guard.** With one machine-bearing design in the module,
  bare feature names and machine states bind; in a multi-machine module, names
  must resolve uniquely or carry the `EnumType.State` qualifier.
- **`transition` fires a machine edge.** Only a delegation to a repository's
  `save` may carry it, and the trigger must belong to the machine bound to that
  repository's design.
- **A body is orchestration-only.** `let`, `let … else error …`, assignment to
  a loaded local's feature path, `guard … else error …`, `if`/`else`, and
  `return`. Loops, local or compound reassignment, collection mutation,
  `try`/`catch`, cross-aggregate loads, and bare expression statements are
  refused by name.

## Constraints

- Clause order is fixed; `when` before `capability` is a syntax error.
- `throws` is complete: raising an error outside it is `E0373`; a delegation
  cannot narrow its target's thrown set (`E0377`).
- A body receiver must be an injected repository designed in the same module
  (`E0359`); every other statement must be in the subset (`E0360`).
- A declared repository operation takes `throws? when?` only — it cannot carry
  capabilities, invocations, emissions, or a transition binding.
- `protected` keeps an operation off the public surface; it is a leading
  modifier only.
- A capability granted on a **non-root** aggregate member warns; capabilities
  on roots, values, and undesignated classes stay silent.

## Examples

```ddd,fixture=examples/support/support.mox
import "support.mox";

application SupportDesk {
    base nz.example.support
    error TicketAlreadyResolved(String ticketNo);

    module desk {
        service TicketService {
            inject TicketRepository;
            boolean resolveTicket(String ticketNo) throws NotFound, TicketAlreadyResolved => TicketRepository {
                let ticket = self.findById(ticketNo) else error NotFound("Ticket", ticketNo);
                guard ticket.state != TicketState.Resolved else error TicketAlreadyResolved(ticketNo);
                ticket.state = TicketState.Resolved;
                let saved = self.save(ticket);
                return true;
            }
            resolve => TicketRepository.save transition Resolve;
        }

        entity Ticket repository TicketRepository {
            findById;
            findAll;
            save;
            delete;
        }
        machine state: TicketState {
            initial Open;
            Open -> InProgress via Claim;
            InProgress -> Resolved via Resolve;
            terminal Resolved;
        }
    }
}
```

`resolveTicket` loads, guards, changes, saves, and returns. `resolve` delegates
to the repository's `save` and binds the machine's `Resolve` edge, so the
delegation and the lifecycle cannot drift apart.

## Diagnostics

| Code | Meaning |
|---|---|
| `E0350` | A `transition` names no trigger of the bound machine. |
| `E0351` | A transition binding on a declared operation, or on a delegation that is not a repository `save`. |
| `E0352` | A multi-machine module requires the qualifier on a transition binding. |
| `E0353`, `E0354` | A condition path is unknown, or a name is ambiguous across the module's machines. |
| `E0355`–`E0358` | A `children` block names an unknown feature, needs a collection, names an unknown action, or targets a non-root class. |
| `E0359` | A body names a receiver the service does not inject. |
| `E0360` | A statement outside the orchestration subset, or a body that does not type. |
| `E0373` | A raised or thrown error is not in the visible vocabulary. |
| `E0377` | A delegation throws a narrower set than its target. |

## Compatibility

- A delegation's target is resolved in the design model; consumers never
  re-resolve a name.
- A body compiles into a checked statement list; the artifact carries the
  error vocabulary and the transition binding alongside it.
- Capabilities are validated against actor policies when the host provides
  them; a design compiled without policies is not silently treated as
  authorized.
