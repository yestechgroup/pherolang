---
title: Case models
description: The syntax and rules of a .cmmn case — roles, case file, plan items, criteria, flags, and decision tasks.
type: reference
language: cmmn
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/cmmn/overview.md]
related: [docs/guide/cmmn/concepts/cases-and-plan-items.md, docs/guide/cmmn/concepts/omg-cmmn-conformance.md]
lastReviewed: 2026-11-21
---

# Case models

This page is the authority for what a `.cmmn` file declares and what each
declaration means. The normative grammar lives in the language's engineering
reference ([grammar](../../../CMMN.md#grammar)); this page states the shape a
reader writes and the rules the compiler enforces.

## Syntax

The complete shape of a case, with all five plan-item kinds, both criteria, the
control flags, and the two automation bindings:

```cmmn,fragment
import "support.mox"
import "support.evt"
import "support.actor"
import "support.dmn"
import "support.ddd" as support

case RefundDesk {
    roles {
        role triage as Agent;
        role finance;
    }
    case_file {
        Ticket[1..1] ticket;
        RefundDecision[0..1] decision;
    }
    discretionary task Escalate {
        performer: finance;
        entry on ticket.update when (ticket.refundCents > 5000);
    }
    service AssessRefund {
        execute support.TicketService.raiseRefund;
        entry on ticket.create;
        required;
    }
    decision AssessAuthority {
        invokes RefundDecisions.AssessRefund version compatible;
        inputs { ticket }
        outputs { decision }
        entry on AssessRefund.complete;
    }
    stage Assessment {
        entry on ticket.create;
        task ReviewTicket {
            performer: triage;
            exit on decision.create;
        }
        task NotifyCustomer {
            performer: triage;
            entry on RefundRequested;
            repeats;
        }
        milestone RefundSettled {
            entry on ReviewTicket.complete;
            required;
        }
    }
}
```

`NotifyCustomer` watches `RefundRequested`, a declared domain event, by its bare
name; the other criteria watch plan-item events and case-file changes. The
service task binds an application operation, and the decision task names a
decision service. The declarations, in the order a case usually writes them:

| Declaration | Form | Meaning |
|---|---|---|
| Import | `import "<path>" ("as" <alias>)?` | Brings a domain, event contract, actor policy, decision model, or application design into the case. Only a design takes an alias. |
| Case | `case <Name> { … }` | One case. A file may declare more than one; names are unique per file. |
| Roles | `roles { role <name> ("as" <Actor>)?; … }` | The case roles a human task's `performer` names, optionally bound to an actor. |
| Case file | `case_file { <Type>[<mult>]? <name>; … }` | The typed material the case works on. |
| Human task | `task <name> { … }` | Work a person performs, with an optional `performer: <role>;`. |
| Service task | `service <name> { execute <alias>.<Service>.<operation>; … }` | Work an application operation performs. |
| Decision task | `decision <name> { invokes <name>( version <pin>)?; inputs { … } outputs { … } … }` | Work a decision service performs. |
| Milestone | `milestone <name> { … }` | A goal, reached through an entry criterion only. |
| Stage | `stage <name> { … }` | A group of plan items, with its own criteria and nested items. |
| Discretionary item | `discretionary task \| service \| decision <name> { … }` | Work a case worker may add at runtime. |
| Entry criterion | `entry on <on-part>(, <on-part>)* (when (<expr>))?;` | What makes the item available. |
| Exit criterion | `exit on <on-part>(, <on-part>)* (when (<expr>))?;` | What ends the item. |
| Control flags | `required;`, `repeats;`, `auto;` | Required work, repetition on completion, automatic start. |

Multiplicities follow the domain model's convention: `[0..1]` and `[1..1]` for a
single optional or required item, `[]` (the same as `[0..*]`) and `[1..*]` for a
collection. An item with no multiplicity is optional and single (`0..1`). Those
four are the shapes the cmmn-rs consumer supports; any other multiplicity is
rejected there, naming the item.

## Semantics

**A case is a namespace of declarations.** Roles, case-file items, and plan
items are declared once and referred to by name. Plan items and case-file items
share one namespace, so a criterion reference is never ambiguous.

**Roles name the audience of human work.** A task's `performer` names a role,
and every task with a `performer` must name a declared one. A role may bind to an
actor from an imported actor policy with `as`; the binding records authorization
metadata and changes nothing about how the case runs.

**Case-file items are typed by the imported domains.** The type resolves against
every provided domain model, so a case over several imports can hold items from
each. The item is stated once in the case and is the only coordination state the
case owns.

**Human tasks are the only items with a performer.** A service or decision task
is performed by a system. Placing `performer:` on one is an error rather than a
silent no-op.

**A service task binds an application operation.** `execute` names a design
alias, a service, and an operation: `execute support.TicketService.raiseRefund`.
Every service task carries exactly one binding, and the alias must be the one an
import declared.

**A decision task binds a decision service.** `invokes` names the service, which
resolves by its last segment — `RefundDecisions.AssessRefund` and
`AssessRefund` name the same service — and may carry `version compatible` or
`version "X.Y.Z"` exactly as an application operation's decision reference does.
The optional `inputs { … }` and `outputs { … }` lists name case-file items of
the same case; they state which material the service reads and which item
receives its result. A decision task takes no performer.

**Criteria are sentries.** An entry criterion makes an item available; an exit
criterion ends it. Each on-part names something that happens:

- a plan-item event — `create`, `enable`, `start`, `complete`, `terminate`,
  `fail`, `occur` — written as `<item>.<event>`;
- a case-file change — `create`, `update`, `delete`, `addChild`, `removeChild` —
  written as `<item>.<transition>`;
- a declared domain event, written as the event's bare name.

All on-parts of one criterion must occur. A `when (...)` part adds a boolean
condition over the case file, checked when the on-parts are satisfied. Inside it
the case file is in scope both directly and through `case_file`, so
`ticket.refundCents` and `case_file.ticket.refundCents` are the same value, and
each plan item is in scope with its boolean transition predicates, so
`ReviewTicket.complete` may be read as a condition.

**A milestone has an entry criterion and no exit criterion.** A milestone is
reached or not reached; it is not ended.

**A stage gates its children.** A child of a stage cannot become available until
the stage is active. `required;` on a child means the stage cannot complete while
that child is incomplete; `required;` on a stage means the enclosing stage waits
for it in the same way.

**By default, availability is not activation.** An entry criterion enables a
human task and a person starts it. `auto;` starts the item as soon as it becomes
available. `repeats;` makes a completed item available again.

**Discretionary items are planned by a person.** They are declared in the plan
but activated by a case worker's decision rather than by a criterion. They can
still carry criteria, which govern the item once it has been added.

## Constraints

| Rule | Consequence when violated |
|---|---|
| Imports must precede the first `case` declaration. | A later import is reported and ignored; the names it was meant to provide stay unresolved. |
| Every import must resolve to content supplied to the compiler. | An import with nothing supplied is an error naming the import. Only design imports take an alias. |
| Case names are unique per file. | A repeated name is an error. |
| Role names are unique per case. | A repeated role name is an error. |
| A role's `as` name must be an actor of an imported actor policy. | An unknown actor is an error naming the role. |
| Case-file types must resolve through the provided domain models. | An unknown type is an error; a type declared by two provided domains is an error naming both. |
| Plan items and case-file items share one namespace. | A duplicate name of either kind is an error naming the item. |
| `performer` is allowed only on a human task and must name a declared role. | `performer` elsewhere is an error; an unknown role is an error with a suggestion. |
| `execute` is allowed only on a service task, exactly once, and must resolve through the aliased design. | A service task without a binding is an error; an unknown alias, service, or operation is an error naming it. |
| `invokes` is allowed only on a decision task, exactly once. | A duplicate clause is a syntax error; a missing one is an error naming the task; a stray one on another kind is an error. |
| `inputs`/`outputs` are allowed only on a decision task, and every name must be a declared case-file item of the same case. | A stray list is an error; an unknown name is an error with a suggestion over the case-file items. |
| Discretionary planning is allowed on tasks, service tasks, and decision tasks. | A discretionary milestone is an error; a discretionary stage is not part of the grammar. |
| A milestone carries entry criteria only. | An exit criterion on a milestone is an error. |
| Every on-part must resolve, and a transition must come from the closed vocabulary. | An unknown item, event, or transition is an error naming it. A bare name that is not a declared domain event is an error. |
| A `when` condition must be boolean. | A condition of another type is an error naming the criterion. |
| An `invokes` name must resolve to a decision service when decision models are provided. | An unknown service is an error with a suggestion over the declared services; an unsatisfiable version pin is an error naming the pin and the available versions. |

## Examples

A case with nothing but a human task, a role, and a case-file item. It is the
smallest complete case worth writing:

```cmmn,fixture=examples/support
import "support.mox"

case RefundDesk {
    roles {
        role triage;
    }
    case_file {
        Ticket[1..1] ticket;
    }
    stage Triage {
        task ReviewTicket {
            performer: triage;
            required;
        }
    }
}
```

A decision task waiting on a case-file change, with the binding and the two name
lists:

```cmmn,fixture=examples/support
import "support.mox"

case RefundDesk {
    case_file {
        Ticket[1..1] ticket;
        RefundDecision[0..1] decision;
    }
    stage Assessment {
        entry on ticket.create;
        decision AssessAuthority {
            invokes RefundDecisions.AssessRefund version compatible;
            inputs { ticket }
            outputs { decision }
            required;
        }
        milestone Settled {
            entry on AssessAuthority.complete;
        }
    }
}
```

The second case names a decision service without importing its decision model.
The service name is then recorded as written and resolved when the model is
provided; with a model present, the same file reports an unknown service by name
and suggests the declared ones.

## Diagnostics

`.cmmn` findings carry no numeric diagnostic codes. Each message names the case
and the item it concerns, and where a declared set exists the help line suggests
from it — a role, a case-file item, or a decision service. The recurring shapes
are the unknown role, the unknown type, the service task without a binding, the
exit criterion on a milestone, the non-boolean `when` condition, and the
unknown decision service.

An unsatisfiable decision-service version pin reports the same message as
`E0252` — `decision service '<name>' is not provided at version "<pin>" invoked
by <context>` — with the versions the provided models declare in its help line.
The `E0252` row in the diagnostic registry documents that shape.

## Compatibility

The compiled case is a versioned JSON document. It carries a `formatVersion`
key, records names as written, and records `when` conditions verbatim;
resolution and condition typing are not part of the document. A file with no
declarations compiles to exactly `{"formatVersion":2}`. The plan is a flat
graph: a stage names its children, and a case's root items are the ones no stage
lists, so a consumer can rebuild the tree without consulting the source.

The cmmn-rs consumer — the case engine integration in Phero Lang — reads the
compiled case together with the domain model and produces the engine's
case-definition JSON. It translates case-file items with their multiplicities,
human tasks as user tasks carrying the role that may take them, service and
decision tasks as process compositions keyed by their binding, and discretionary
tasks as planning tables. `when` conditions are translated into the engine's
expression language within a documented subset; a condition outside that subset
is rejected with an error naming the criterion, never silently changed.

Two constructs the language accepts are rejected by that consumer, also with
named errors: a bare domain-event on-part, because the engine's on-parts are
plan-item and case-file transitions only, and a plan-item predicate inside a
`when` condition, because the engine evaluates the if-part against the case file
alone. A third documented limit is that the binding between `inputs`/`outputs`
and a decision service's declared types is by name: there is no type-agreement
check, because the runtime that executes the service owns the value mapping.
