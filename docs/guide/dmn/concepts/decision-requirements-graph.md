---
title: The decision-requirements graph
description: How inputs, knowledge, decisions, and services compose into a graph — and what each requirement edge means.
type: concept
language: dmn
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md]
related: [docs/guide/dmn/reference/decision-models.md, docs/guide/dmn/reference/decision-services.md]
lastReviewed: 2026-11-21
---

# The decision-requirements graph

A decision model is a graph, not a list. Its nodes are inputs, knowledge,
decisions, and services; its edges are requirements, and they say what each
decision depends on.

## Overview

The graph has four node kinds:

| Node | Declared as | Meaning |
|---|---|---|
| Input | `inputs { name: Type; }` | Data a decision consumes, typed by the domain model. |
| Knowledge source | `source Name { … }` | An authority a decision answers to, carried as a label. |
| Business knowledge model | `model Name { parameters … output … logic … }` | A reusable rule applied through a decision. |
| Decision | `decision Name { output(s) … logic … }` | A unit of logic with an output contract. |

A **decision service** is not a fifth kind of logic: it is a boundary that names
the inputs a caller supplies and the decisions it receives, with everything
inside the boundary encapsulated.

## Why does it matter?

Rules are rarely independent. Eligibility feeds authority; authority feeds the
final assessment; a regulated decision answers to a named act. Writing those
dependencies down is what makes the model reviewable: a change to one decision's
contract shows which decisions downstream consume it, and a cycle is a compile
error rather than a runtime surprise.

The graph also makes the model auditable. A knowledge source is a first-class
node, so "which decisions does this act govern?" is a query, not a search
through comments.

## How it works

The graph's edges are the three requirement kinds, declared in one
`requirements` block:

- **Information** — `A requires B;` — A needs B's data or result. The left side
  is a decision; the right side is an input or another decision. These edges are
  what a table decision's input signature follows, and they must be acyclic.
- **Knowledge** — `A invokes M;` — A applies the business knowledge model M. The
  decision's `logic invoke` states the call; the requirement states the
  dependency.
- **Authority** — `A governed_by S;` — A answers to the knowledge source S.

A decision service then names a slice of the graph: the inputs a caller
provides, the decisions it receives, and everything the boundary encapsulates.
Callers bind to that boundary, not to the individual decisions.

## Example

```dmn,fixture=examples/support
import "support.mox";

decision_model RefundDecisions {
    namespace nz.example.support.decisions;
    version "1.0.0";

    inputs {
        ticket: Ticket;
        requested_cents: int;
    }

    knowledge {
        source ConsumerGuaranteesAct {
            name "Consumer Guarantees Act 1993";
            reference "https://example.org/cga";
        }
    }

    decisions {
        decision RefundEligibility {
            output boolean;
            logic expression "ticket.needsRefund and requested_cents > 0"
        }
        decision RefundAuthority {
            outputs {
                requires_approval: boolean;
                approver_role: string;
                maximum_amount: int;
            }
            logic table "./tables/refund-authority.md"
        }
    }

    requirements {
        information {
            RefundEligibility requires ticket;
            RefundEligibility requires requested_cents;
            RefundAuthority requires requested_cents;
        }
        authority {
            RefundAuthority governed_by ConsumerGuaranteesAct;
        }
    }

    views {
        view refund_flow {
            description "How a refund request is assessed.";
            show {
                RefundEligibility
                RefundAuthority
                ConsumerGuaranteesAct
            }
            direction left_to_right;
        }
    }
}
```

Two decisions, one input pair, one authority, and a view. The view declares
presentation intent — what to show and in which direction — and nothing about
coordinates or edges; the requirements already carry the structure.

## Relationship to other concepts

- The **domain model** ([.mox](../mox/overview.md)) supplies every input and
  output type; DMN has no separate type definitions.
- An **application design** ([.ddd](../ddd/overview.md)) invokes a decision
  service from an operation, with an optional version pin.
- An **interaction flow** ([.ifml](../ifml/overview.md)) may bind a component's
  `data:` to a decision service.
- An **event contract** ([.evt](../../../EVENTS.md)) declares the events a
  decision's `emits` block names.
- A **case model** ([.cmmn](../../../CMMN.md)) invokes decision services from
  decision tasks.

## Further reading

- [Decision models](../reference/decision-models.md) — the full syntax and the
  numbered validation rules.
- [Decision services](../reference/decision-services.md) — the invocation
  boundary and versioning.
- [OMG DMN conformance](omg-dmn-conformance.md) — how the graph relates to the
  standard.
