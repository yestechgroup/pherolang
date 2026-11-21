---
title: Enums, datatypes, and vocabularies
description: Declare closed sets of literals, opaque wrapped types, and external code sets — and know what each one puts on the wire.
type: reference
language: mox
section: reference
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md]
related: [docs/guide/mox/how-to/add-an-enum.md, docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Enums, datatypes, and vocabularies

Three declaration kinds extend the type universe beyond primitives and classes:
an **enum** closes a set of values, a **datatype** wraps a platform type
opaquely, and a **vocabulary** references a well-known external vocabulary.

## Syntax

```text
enum <Name> {
    <Literal> ("as" "<label>")? "=" <integer>
}

type <Name> wraps ("opaque" | <type>)? {
    <target> "<platform type>"
    format "<json-schema-format>"
    create { <target-body> }
    convert { <target-body> }
}

vocabulary <Name> from "<source>" {
    version "<version>"
    key <facet>
    facet <Type> <name>
}
```

```mox,fragment
enum OrderState {
    Draft as "draft" = 0
    Placed as "placed" = 1
}

type Email wraps String {
    format "email"
}
```

## Semantics

**Enums.** A literal has a required integer value and a name; the optional
`as "…"` label is display metadata. The **name** is the wire spelling:
canonical JSON and the generated JSON Schema carry `Draft`, and generated code
exposes the literal's name, label, and integer value. The integer is the stored
value and must be explicit — a literal without one is an error.

**Datatypes.** A datatype wraps one platform type opaquely. The block carries
per-target binding hints, which are hints only and never generate dependencies.
At most one `format` entry is allowed; it is a reserved key and surfaces as the
JSON Schema `format` keyword. A datatype may also declare at most one `create`
and one `convert` entry — target-tagged bodies carried next to the bindings.

**Vocabularies.** A vocabulary names the source, version, key facet, and typed
facets of an external vocabulary. Its snapshot is vendored to
`vocab/<source>@<version>.json` and pinned by sha256 in `model.lock`, so
compilation is hermetic; only `pherolang vocab fetch` touches the network.
Entries are embedded in the model, and every consumer materialises the
vocabulary as a closed set of keys with facet accessors.

## Constraints

- An enum literal requires an explicit integer value.
- A declared type shadows the like-named built-in: a datatype named `Date`
  shadows the `date` primitive (write lowercase `date` for the calendar type).
- `date` attributes take no declared default and admit no constraint keywords.
- An enum-typed attribute accepts both constraint families: the string family
  bounds literal names, the numeric family bounds literal values. A numeric or
  length bound that admits zero literals is an error.
- A datatype-typed attribute defaults to the string constraint family and
  rejects numeric bounds.
- A vocabulary-typed attribute follows the `key` facet's declared primitive
  type; its declared constraints are rejected when the key facet does not
  resolve.
- A vocabulary compiles only when its pinned snapshot is present; a missing
  snapshot is the fetch error, not a silent empty set.
- `format` is reserved inside a datatype block and may appear at most once.

## Examples

```mox
package nz.example.orders

enum OrderState {
    Draft as "draft" = 0
    Placed as "placed" = 1
    Cancelled as "cancelled" = 2
}

type Email wraps String {
    format "email"
}

class Order {
    id readonly String orderId
    OrderState state = Draft
    Email customerEmail
    int totalCents { minimum 0 }
}
```

The order carries a lifecycle value from a closed set and a validated contact
address, with the total bounded below. The label `"draft"` is for display;
canonical JSON stores `"Draft"`.

## Diagnostics

| Situation | Report |
|---|---|
| A literal has no `= <integer>` | A parse error at the literal: an explicit value is required. |
| A stripped name collides with another declaration | `E0300` — enums share the class module-name rule. |
| A constraint family does not fit the type | The constraint error naming the family and the feature (see [Classes and features](classes.md)). |

## Compatibility

- The literal **name** is the wire value in canonical JSON and JSON Schema; the
  label is not.
- A datatype's `format` hint is a wire-level JSON Schema keyword; no runtime
  validation is implied.
- Vocabulary snapshots are pinned by sha256 in `model.lock`; a version bump is a
  lockfile change, reviewed like any other.
