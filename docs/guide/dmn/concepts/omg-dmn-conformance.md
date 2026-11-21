---
title: OMG DMN conformance
description: How Phero Lang's decision models relate to OMG DMN 1.6 — what is implemented, extended, changed, and left out.
type: concept
language: dmn
section: concepts
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md]
related: [docs/guide/dmn/concepts/decision-requirements-graph.md, docs/guide/dmn/reference/decision-models.md]
lastReviewed: 2026-11-21
---

# OMG DMN conformance

Phero Lang's decision models follow the DMN decision architecture. This page
states what that means concretely, using the classification the editorial policy
requires. Every claim below is one of:

- **industry standard**
- **Phero implementation**
- **Phero extension**
- **Phero divergence**
- **proposed feature**

## Overview

OMG DMN 1.6 defines a decision architecture — a decision-requirements graph of
decisions, inputs, business knowledge models, knowledge sources, and decision
services, with rules expressed in FEEL or decision tables — usually expressed as
XML. Phero Lang implements that architecture as a text language, with the
domain model as its type universe.

The short version: **the decision architecture is the standard's; the surface is
not XML, and several implementation choices are deliberate.** Phero Lang does
not claim DMN XML interchange or full conformance to the specification.

## Why does it matter?

"DMN support" can mean anything from "imports DMN XML" to "uses the word
decision". A reader choosing a decision platform needs to know which of those
they are getting. Stating the supported, extended, divergent, and absent parts
lets them evaluate the fit and lets a future proposal argue from the actual
position.

## How it works

| Classification | Concepts |
|---|---|
| Industry standard | The DMN decision architecture: decision-requirements graphs, decisions with output contracts, input data, business knowledge models, knowledge sources, decision services, the three requirement kinds, and hit policies. |
| Phero implementation | Each of those as a `.dmn` declaration, typed by the imported `.mox` domain; FEEL expression texts are syntax-checked at compile time. |
| Phero extension | External markdown decision tables; `emits` mapping a decision outcome to declared `.evt` events; `.ddd`/`.cmmn` version pins on service invocation; decision data in `.ifml`; views as presentation intent. |
| Phero divergence | A text surface, not XML; FEEL is stored and syntax-checked, not executed; types come from `.mox`, not FEEL `itemDefinition`; no DRD/DI diagram interchange; no evaluation engine in the surface. |
| Unsupported (v1) | DMN XML model interchange; boxed expressions inside the file; diagram interchange; evaluation. |
| Proposed feature | Nothing in this surface is presented as implemented ahead of its gate; proposed work is tracked as proposals. |

Two practical consequences:

- A decision model exported as DMN XML cannot be imported, and a Phero Lang
  model cannot be exported to DMN XML. Alignment is conceptual; re-authoring is
  the migration path.
- A tool expecting FEEL evaluation must use a consumer that lowers the model
  (the in-tree decision integration does, by deliberate choice), not the
  surface.

## Example

The decision architecture written as Phero Lang declarations: one input, one
decision with an output contract, one rule.

```dmn,fixture=examples/support
decision_model Approval {
    namespace nz.example.approval;

    inputs {
        amount: int;
    }

    decisions {
        decision auto_approve {
            output boolean;
            logic expression "amount <= 5000"
        }
    }
}
```

A decision with an output contract and an expression rule — the standard's
concept, written as Phero Lang declarations. With an imported domain the input
could be a domain class instead of a primitive.

## Relationship to other concepts

- [The decision-requirements graph](decision-requirements-graph.md) describes
  the architecture this page relates to the standard.
- [Decision models](../reference/decision-models.md) is the syntax reference,
  including the numbered validation rules.
- The engineering reference's [deviations from OMG DMN
  1.6](../../../../DMN.md#deviations-from-omg-dmn-16) carries the complete list
  with rationale.

## Further reading

- [The .dmn language reference — Conformance](../../../../DMN.md#conformance) —
  the canonical fixture set and what it pins.
- [External decision tables](../../../../DMN.md#external-decision-tables) — the
  table shape and its division of labour.
- [Decision services](../reference/decision-services.md) — the invocation
  contract.
