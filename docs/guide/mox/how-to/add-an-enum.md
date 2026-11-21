---
title: Add an enum to a domain model
description: Model a feature whose value comes from a fixed set, using an enum with stable integer values and readable labels.
type: how-to
language: mox
section: how-to
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md, docs/guide/mox/tutorials/your-first-domain-model.md]
related: [docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Add an enum to a domain model

Use this page when a feature's value must come from a fixed set — a state, a
category, a code — and you want anything outside the set to be rejected.

## Problem

A string attribute accepts any text. If `status` may only be `draft`, `sent`,
or `cancelled`, a plain `String status` cannot express that, and the rule ends
up repeated in every piece of code that reads it. You need one declaration that
holds the set and a feature that uses it.

## Prerequisites

- A `.mox` file that already has a `package` declaration. If you do not have
  one, the [tutorial](../tutorials/your-first-domain-model.md) builds the file
  this page edits.
- Basic familiarity with declaring a class.

## Solution

**1. Declare the enum at the top level of the package.** Each literal is a name
with a required integer value; the `as "…"` label is optional.

```mox,fragment
enum InvoiceState {
    Draft as "draft" = 0
    Sent as "sent" = 1
    Cancelled as "cancelled" = 2
}
```

The integer is the stored value. The literal name is the wire spelling:
canonical JSON and the generated schema use `Draft`, not the label. The optional
`as "…"` label is a human-readable string carried into generated code.

**2. Use the enum name as a feature type.** An enum is a type, exactly like a
class or an attribute's primitive type:

```mox,fragment
class Invoice {
    id readonly String invoiceNo
    InvoiceState state = Draft
}
```

**3. Set a default if every instance starts in the same state.** The default is
the literal's name, not its integer:

```mox,fragment
InvoiceState state = Draft
```

Without a default the feature is required when an instance is constructed.

**4. Check the model.**

```sh
pherolang check domain.mox
```

A literal missing its value, or a feature typed with an undeclared enum name, is
reported at the line that caused it.

## Example

```mox
package nz.example.billing

enum InvoiceState {
    Draft as "draft" = 0
    Sent as "sent" = 1
    Cancelled as "cancelled" = 2
}

class Invoice {
    id readonly String invoiceNo
    int totalCents { minimum 0 }
    InvoiceState state = Draft
}
```

`Invoice.state` now accepts only the three declared literals. The literal is
written `Draft` in the model and serialized as `"Draft"`; the label `"draft"` is
display metadata, never the wire value.

## Related reference

- [Classes and features](../reference/classes.md) — the full feature reference,
  including constraints on enum-typed attributes.
- [Enums, datatypes, and vocabularies](../reference/enums-datatypes-and-vocabularies.md)
  — the authoritative page for enum values, labels, and the rest of the type
  universe.
- [The .mox language reference](../../../LANGUAGE.md#enums-datatypes-interfaces-vocabularies)
  — the authoritative enum rules.
