---
title: Cases and plan items
description: How a case declares the work available around a matter, and how criteria decide when each item becomes available.
type: concept
language: cmmn
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/cmmn/overview.md]
related: [docs/guide/cmmn/concepts/omg-cmmn-conformance.md, docs/guide/cmmn/reference/case-models.md]
lastReviewed: 2026-11-21
---

# Cases and plan items

A case is one matter in play — a claim, an application, a refund — plus the work
that must be done about it. A **plan item** is one piece of that work. The two
ideas carry the whole language: everything else says who may do an item, what
material it touches, and what makes it possible.

## Overview

A case has a **case file**: the typed material the work reads and changes. It
has **plan items**: human tasks, service tasks, decision tasks, milestones, and
stages that group other items. And it has **criteria**: the entry criterion that
makes an item available and the optional exit criterion that ends it.

That is the difference from a scripted process. A script says *do this, then
that*. A case says *this is available, and here is what makes it available*. The
ordering is a consequence of the criteria, not a separate decision that somebody
has to maintain.

## Why does it matter?

Real coordination is conditional. A review can start as soon as a ticket exists,
but an approval can only start once the review lands, and only for refunds above
a threshold. Written as a sequence, those conditions become positions in a list:
a step moves up or down whenever a threshold changes, and two steps that could
run at the same time are forced to wait for each other.

Written as criteria, each condition sits next to the work it governs. Adding a
second reviewer is a task with an entry criterion, not a renumbering of the
process. Removing a step removes the task and leaves the rest untouched.

The case file matters for the same reason. When coordination state lives in the
business object, every consumer of that object inherits the case's bookkeeping.
Keeping the material in the case file leaves the object's own lifecycle to the
domain model, where the domain's rules already live.

## How it works

**The case file holds typed items.** A case-file item is a domain class with a
multiplicity — `Ticket[1..1] ticket`, `RefundDecision[0..1] decision`. The
multiplicity states whether the item must be present and whether there may be
more than one. An item that is not there yet is normal: cases acquire material
as they run.

**Plan items and case-file items share one namespace.** Criteria name both kinds,
so a single namespace keeps every reference unambiguous. A name cannot be reused
for a second item of either kind.

**Stages group work.** A stage is the only plan item that contains other items.
The stage's own entry criterion gates everything inside it: until the stage is
active, its children cannot become available. A child marked `required` blocks
the stage from completing while it is incomplete.

**Entry criteria enable; they do not start.** By default a human task becomes
available when its entry criterion is satisfied, and a person starts it. The
`auto` flag removes that manual step and starts the item as soon as it is
available. This is the standard's manual activation default; the
[conformance page](omg-cmmn-conformance.md) classifies it as industry standard.

**Exit criteria end work.** A task that should stop when its purpose is served
carries an exit criterion — for example, a review task that ends as soon as a
decision is recorded. Without one, the task ends when the person completes it.

**An on-part names something that happens.** It is one of three things: a plan
item's event (`ReviewTicket.complete`, `Reviewed.occur`), a case-file change
(`ticket.create`, `decision.update`), or the name of a declared domain event. A
criterion with several on-parts is satisfied only when all of them have
occurred.

**A `when` condition narrows the trigger.** The optional `when (...)` part is a
boolean expression over the case file. It is checked when the on-parts are
satisfied, so the same event can activate an item in one case and not in
another.

**Discretionary work is planned by a person.** A discretionary task is declared
in the plan but not activated by criteria alone: a case worker decides whether
to add it. It is how a case acknowledges that some responses are a matter of
judgement.

**Flags state control rules.** `required` marks work the surrounding stage
cannot finish without. `repeats` lets a completed item become available again.
`auto` starts an item without waiting for a person.

## Example

A refund desk in which the intake stage is gated on the ticket actually needing
a refund, the review ends when a decision is recorded, and approval is required
only above a threshold.

```cmmn,fixture=examples/support
import "support.mox"

case RefundDesk {
    roles {
        role triage;
        role finance;
    }
    case_file {
        Ticket[1..1] ticket;
        RefundDecision[0..1] decision;
    }
    stage Intake {
        entry on ticket.create when (ticket.refundCents > 0);
        task ReviewTicket {
            performer: triage;
            exit on decision.create;
        }
        milestone Reviewed {
            entry on ReviewTicket.complete;
        }
        task ApproveRefund {
            performer: finance;
            entry on Reviewed.occur when (ticket.refundCents > 5000);
        }
    }
}
```

The stage becomes active when the ticket is created and needs a refund. The
review is available immediately afterwards, and ends when a decision exists.
`Reviewed` is reached when the review completes, so `ApproveRefund` becomes
available — but only for refunds above 5000 cents. Nothing states the order
between the review and the approval: the approval's entry criterion does.

## Relationship to other concepts

- A [state machine](../../ddd/reference/state-machines.md) owns the legal states
  and transitions of one object. The case owns the work around that object; the
  object's state changes only through the domain's own operations.
- A [decision model](../../dmn/overview.md) owns the rules that map inputs to an
  outcome. A decision task invokes a decision service; the rules stay in the
  decision model.
- An event contract owns the domain events a criterion may watch by name. The
  case reacts to them; it does not declare them.
- An actor policy owns who may do what in the application. A case role may bind
  to an actor, so the case's audience and the policy's subject are the same
  party.
- An [interaction flow](../../ifml/overview.md) owns the screens a performer
  works through. The case names the work; the flow presents it.

## Further reading

- [OMG CMMN conformance](omg-cmmn-conformance.md) — which parts of the standard
  these concepts come from.
- [Case models](../reference/case-models.md) — the syntax of every declaration
  and the rules each must satisfy.
- [The .cmmn language reference](../../../CMMN.md) — the complete engineering
  reference.
- The [support-desk case](../../../../examples/support/support.cmmn) puts every
  declaration to work over the support domain.
