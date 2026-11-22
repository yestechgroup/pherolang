---
title: OMG CMMN conformance
description: How Phero Lang's case models relate to OMG CMMN 1.1 — what is implemented, extended, changed, and left out.
type: concept
language: cmmn
section: concepts
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/cmmn/overview.md]
related: [docs/guide/cmmn/concepts/cases-and-plan-items.md, docs/guide/cmmn/reference/case-models.md]
lastReviewed: 2026-11-21
---

# OMG CMMN conformance

Phero Lang's case models follow the CMMN notion of case management. This page
states what that means concretely, using the classification the editorial policy
requires. Every claim below is one of:

- **industry standard**
- **Phero implementation**
- **Phero extension**
- **Phero divergence**
- **proposed feature**

## Overview

OMG CMMN 1.1 (formal/2016-06-01) defines case management: a case file of
material, a plan of work that becomes available through criteria rather than
sequence, sentries with entry and exit criteria, milestones as goals,
discretionary items a case worker adds, and case roles. Phero Lang implements
that architecture as a text language, with the domain model as its type
universe.

The short version: **the case architecture is the standard's; the surface is
not XML, and several choices are deliberate.** Phero Lang does not claim CMMN
XML interchange or full conformance to the specification.

## Why does it matter?

"CMMN support" can mean anything from "imports CMMN XML" to "uses the word
case". A reader choosing a case platform needs to know which of those they are
getting. Stating the implemented, extended, divergent, and absent parts lets
them evaluate the fit and lets a future proposal argue from the actual position.

It also keeps two different questions apart. Whether a case *means* the same
thing as a CMMN case is a modelling question about plans, sentries, and roles.
Whether the file can be exchanged with a CMMN tool is an interchange question.
An honest answer to the first does not imply the second.

## How it works

| Classification | Concepts |
|---|---|
| Industry standard | The case as a unit of adaptive, condition-driven work: a case file of material, plan items that become available through criteria, stages that group them, milestones as goals, discretionary items, repetition, manual activation by default, the required rule, and case roles. |
| Phero implementation | Each of those as a declaration in a `.cmmn` file, typed by the imported domain model: case-file items with multiplicities, entry and exit criteria carrying on-parts plus an optional boolean `when` condition, `performer:` naming a case role, `execute` binding a service task to an application operation, and `invokes` binding a decision task to a decision service. |
| Phero extension | The domain model as the type universe; a bare event name as an on-part, watching a declared domain event; `inputs`/`outputs` name lists and version pins on a decision task; a role's `as` binding to an actor policy. |
| Phero divergence | A text surface rather than XML; control rules stated as flags (`required`, `repeats`, `auto`) rather than as general rule expressions; a closed plan-item and case-file transition vocabulary; service tasks bound to a Phero Lang operation rather than to an external process. |
| Unsupported (v1) | CMMN XML and XMI interchange; diagram interchange; a case task that invokes another case; discretionary stages; the suspend, resume, and parentTerminate plan-item transitions and the case-file replace transition; authoring table items and applicability rules in the plan. |
| Proposed feature | Nothing in this surface is presented as implemented ahead of its gate; proposed work is tracked as proposals. |

Two practical consequences:

- A case exported as CMMN XML cannot be imported, and a Phero Lang case cannot
  be exported to CMMN XML. Alignment is conceptual; re-authoring is the
  migration path.
- A tool that expects to execute a case needs a consumer that translates the
  compiled case into its own engine's model. The cmmn-rs consumer does that by
  deliberate choice; the `.cmmn` file on its own is not an executable
  definition for any engine.

## Example

The standard's concepts written as Phero Lang declarations: a stage, a human
task with a role, two sentries, and a milestone.

```cmmn,fixture=examples/support
import "support.mox"

case RefundDesk {
    roles {
        role triage;
    }
    case_file {
        Ticket[1..1] ticket;
        RefundDecision[0..1] decision;
    }
    stage Assessment {
        entry on ticket.create;
        task ReviewTicket {
            performer: triage;
            exit on decision.create;
        }
        milestone Reviewed {
            entry on ReviewTicket.complete;
        }
        task RecordDecision {
            performer: triage;
            entry on Reviewed.occur when (ticket.refundCents > 0);
        }
    }
}
```

`Assessment` is a stage; `ReviewTicket` is a human task carrying a case role; its
exit criterion watches a case-file change, and `Reviewed` is a milestone reached
by a plan-item event. The `when` condition on `RecordDecision` is the if-part of
a sentry, written as a typed Phero Lang expression.

## Relationship to other concepts

- [Cases and plan items](cases-and-plan-items.md) describes the architecture
  this page relates to the standard.
- [Case models](../reference/case-models.md) is the syntax reference, including
  the rules each declaration must satisfy.
- The engineering reference's [resolution and validation
  rules](../../../CMMN.md#resolution--validation) carry the complete list with
  their consequences.
- [OMG DMN conformance](../../dmn/concepts/omg-dmn-conformance.md) applies the
  same classifications to the sibling decision language.

## Further reading

- [The .cmmn language reference — Conformance](../../../CMMN.md#conformance) —
  the canonical fixture set and what it pins.
- [The .cmmn language reference — Tooling](../../../CMMN.md#tooling) — the
  commands that format and check a case.
- The [support-desk case](../../../../examples/support/support.cmmn) — the
  full-stack example, whose imports span the domain, the events, the policy, the
  decision model, and the application design.
