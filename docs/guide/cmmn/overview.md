---
title: Case management (.cmmn)
description: Declare a case — the adaptive work coordinated around a domain, made available by criteria instead of a fixed sequence.
type: overview
language: cmmn
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/cmmn/concepts/cases-and-plan-items.md, docs/guide/cmmn/reference/case-models.md]
lastReviewed: 2026-11-21
---

# Case management (.cmmn)

A `.cmmn` file declares a **case**: the work an organisation coordinates around
one matter while that matter is in play. A refund request, a loan application,
and an insurance claim are cases. The file names the people involved, the
material they work on, the goals to reach, and the conditions under which each
piece of work becomes available — it does not script the order in which the work
happens.

## What is it?

A case is a plan of *available* work. You declare **roles** (the people a task
can be given to), a **case file** (the typed material the work reads and
changes), and **plan items**: human tasks, automated tasks, milestones, and
stages that group them. Each item can carry **criteria** — an entry criterion
that makes it available and an optional exit criterion that ends it. Nothing
runs because it is next in a list. An item runs when its criterion is satisfied
and, for human work, when a person takes it.

The language follows the case management model of CMMN 1.1, published by the
OMG, and writes it as text rather than XML. The
[conformance page](concepts/omg-cmmn-conformance.md) states exactly which parts
of the standard are implemented, extended, and left out.

A case is not a state machine, and it is not a prescribed process. A state
machine owns the legal states and transitions of one object. A prescribed
process fixes the order of every step. A case owns neither: it declares what
work exists and what makes it possible, and lets the order emerge.

## What problems does it solve?

Some work cannot be planned in advance. A refund desk does not know whether a
ticket needs a second reviewer until the first review is done. A lender does not
know which financial statements will arrive, or when. Teams run this work
anyway, in queues and spreadsheets, and the plan lives in people's heads.

Forcing that work into a fixed sequence has two failure modes. If the sequence
is strict, work that could proceed waits for a step that does not matter yet. If
the sequence is loose, work starts before its inputs exist and the correction
costs more than the wait. A case states the real dependencies — *this task
becomes available when that review completes and the ticket still needs a
refund* — and leaves the rest concurrent.

A second problem is ownership. Work around a business object is often modelled
by widening the object's own lifecycle until it carries the coordination as
well. The case keeps the two apart: the domain model owns the state of the
ticket, and the case owns the work around it.

## Key capabilities

- **A typed case file.** Declare the material the case works on — `Ticket`,
  `RefundDecision` — typed by the domain model, with the multiplicity each item
  needs.
- **Roles.** Name the parties a human task can be given to, and bind a role to
  an actor policy for authorization metadata.
- **Human tasks.** Declare work a person performs, with the role that may take
  it.
- **Automated work.** Bind a service task to an application operation, or a
  decision task to a decision service.
- **Stages.** Group related work under one entry criterion, and mark the items a
  stage cannot complete without.
- **Milestones.** Declare a goal that is reached when its entry criterion is
  satisfied.
- **Criteria with conditions.** Watch plan-item events, case-file changes, or
  declared domain events, and add a typed boolean condition over the case file.
- **Discretionary work and control flags.** Let a case worker add work at
  runtime, and state whether an item is required, repeats, or starts
  automatically.

## A simple example

A refund desk with two stages: intake reviews the ticket, and settlement closes
it once a decision is on file.

```cmmn,fixture=examples/support
import "support.mox"

case RefundDesk {
    roles {
        role triage;
        role finance;
    }
    case_file {
        /// The ticket under triage.
        Ticket[1..1] ticket;
        /// The refund decision once one is on file.
        RefundDecision[0..1] decision;
    }
    stage Intake {
        entry on ticket.create;
        task ReviewTicket {
            performer: triage;
        }
        discretionary task Escalate {
            performer: finance;
        }
        milestone Reviewed {
            entry on ReviewTicket.complete;
        }
    }
    stage Settlement {
        entry on Reviewed.occur;
        task CloseTicket {
            performer: triage;
            exit on decision.create;
            required;
        }
    }
}
```

`ReviewTicket` is available as soon as the case has a ticket; `Reviewed` is
reached when the review completes; `Settlement` opens only then, and its closing
task ends when a decision is recorded. `Escalate` is discretionary: it is in the
plan, but a case worker decides whether to add it.

## How it fits into Phero Lang

`.cmmn` adds the work dimension to the modelling ecosystem and owns nothing
else. The types, the object lifecycles, the decision rules, and the events stay
in their own languages.

- [`.mox`](../mox/overview.md) supplies the types every case-file item resolves
  against.
- [`.ddd`](../ddd/overview.md) supplies the aggregate lifecycle and the service
  operations a service task binds to. The lifecycle and the case stay separate:
  **the domain owns its state; the case owns its work.**
- [`.dmn`](../dmn/overview.md) supplies decision services a decision task
  invokes, with the same version pins an application operation uses.
- [`.evt`](../../EVENTS.md) declares the events a criterion may watch by name,
  when the trigger is a domain event rather than a change in the case.
- [`.actor`](../../LANGUAGE.md) declares the actors a role may bind to, so a
  human task's audience is the same party the policy authorizes.
- [`.ifml`](../ifml/overview.md) presents the work to the people the case names.
- [`.deploy`](../../DEPLOY.md) places the application, and the engine that
  executes the case, on a runtime target.

A lending application illustrates the split: `.mox` models the borrower and the
loan, `.ddd` the loan's lifecycle and services, `.dmn` the credit decision, and
`.cmmn` the underwriting work around a single application.

## Getting started

This guide tree has no step-by-step tutorial yet. The concept pages are the
shortest path into the language:

- [Cases and plan items](concepts/cases-and-plan-items.md) explains how a case
  declares work and how criteria decide when it becomes available.
- [OMG CMMN conformance](concepts/omg-cmmn-conformance.md) states how the
  language relates to the standard.
- [Case models](reference/case-models.md) is the syntax reference to write
  against.

The [support-desk case](../../../examples/support/support.cmmn) is a complete
case the project compiles on every change.

## Further reading

- [Cases and plan items](concepts/cases-and-plan-items.md) — the concept behind
  the declarations.
- [OMG CMMN conformance](concepts/omg-cmmn-conformance.md) — the classification
  the editorial policy requires.
- [Case models](reference/case-models.md) — every declaration and its rules.
- [The .cmmn language reference](../../CMMN.md) — the complete engineering
  reference, including the grammar and the consumer contract.
