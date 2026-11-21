---
title: OMG IFML conformance
description: How Phero Lang's interaction flows relate to OMG IFML 1.0 — what is implemented, extended, changed, and left out.
type: concept
language: ifml
section: concepts
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/ifml/overview.md]
related: [docs/guide/ifml/concepts/views-and-navigation.md, docs/guide/ifml/reference/views-and-components.md]
lastReviewed: 2026-11-21
---

# OMG IFML conformance

Phero Lang's interaction flows are aligned with OMG IFML 1.0. This page states
what that alignment means concretely, using the classification the editorial
policy requires. Every claim below is one of:

- **industry standard**
- **Phero implementation**
- **Phero extension**
- **Phero divergence**
- **proposed feature**

## Overview

OMG IFML 1.0 defines a metamodel for interaction flows: views, view
containers, components, events, navigation flows, and data binding, usually
expressed as UML/XMI. Phero Lang implements that conceptual model as a text
language and checks it against a domain model.

The short version: **the concepts come from the standard; the syntax,
expressions, and several additions belong to Phero Lang.** "Aligned with IFML" is a statement about
meaning, not about XML interchange, and Phero Lang does not claim full
conformance to the specification.

## Why does it matter?

A conformance claim is a promise a reader can check. Without one, "supports
IFML" means whatever the writer hoped: sometimes "uses similar words",
sometimes "imports OMG's XMI". Stating the supported, extended, divergent, and
absent parts lets a reader decide whether Phero Lang fits their situation — and
lets a proposal to close a gap argue with the actual position.

## How it works

| Classification | Concepts |
|---|---|
| Industry standard | The IFML metamodel's vocabulary: views, view containers, components, events, navigation flows, data binding, actions. |
| Phero implementation | Each of those concepts as a `.ifml` declaration, with the standard's meaning: a view is a screen or interface unit, a navigation flow connects views, a component is an interface element bound to data. |
| Phero extension | Typed authorization references (`requires:`, `roles:`); decision-data bindings to decision services; CLI `command` declarations; the reference pattern library; the derived control/route manifest. |
| Phero divergence | Text as the source of truth instead of XMI/XML; a C-like expression syntax instead of the OMG expression language; diagrams are derived views, not the model; no UML model binding in v1. |
| Unsupported (v1) | OMG's XMI interchange; a visual diagram editor; binding to an external UML metamodel. |
| Proposed feature | Deferred CLI ideas (modality profiles, doc/completion generation, TUI constructs) are tracked as proposals, never presented as implemented. |

Two consequences matter in practice:

- An IFML model exported as XMI cannot be imported. The alignment is at the
  concept level; re-authoring is the migration path.
- A tool that expects the OMG expression language must translate the Phero Lang
  expression syntax; the grammar is documented in the engineering reference.

## Example

The concepts the standard defines, written as Phero Lang declarations:

```ifml,fixture=examples/support
import "support.mox";

view "Tickets" {
    label "Ticket list";

    component "list" {
        type: list;
        data: Ticket;
        fields: [ticketNo, title];

        on select(row) -> navigate("TicketDetail", {
            ticket: row.ticketNo
        });
    }
}

view "TicketDetail" {
    params { ticket: Ticket };

    component "details" {
        type: details;
        data: Ticket;
    }
}
```

A view, a component, an event, a navigation flow, and a data binding — the
standard's concepts, written as Phero Lang declarations. The `params` block and
the `on select` payload are this language's spelling of the flow's parameter
binding.

## Relationship to other concepts

- [Views and navigation](views-and-navigation.md) documents the model this page
  relates to the standard.
- [Views and components](../reference/views-and-components.md) is the syntax
  reference, including the extensions.
- [The .ifml language reference](../../../../IFML.md) is the engineering
  contract, including the pattern library and the manifest.

## Further reading

- [The .ifml language reference — Philosophy](../../../../IFML.md#philosophy) —
  the design principles behind the alignment.
- [Syntax examples](../../../../IFML.md#syntax-examples) — CRUD, wizards,
  dashboards, and CLI flows.
- [Events and navigation](../reference/events-and-navigation.md) — the effects
  vocabulary.
