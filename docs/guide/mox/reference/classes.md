---
title: Classes and features
description: The reference for declaring a class and every kind of feature it can carry — attributes, relationships, derived values, and operations.
type: reference
language: mox
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md]
related: [docs/guide/mox/concepts/modelling-not-coding.md, docs/guide/mox/how-to/add-an-enum.md]
lastReviewed: 2026-11-21
---

# Classes and features

A class declares a kind of thing in the domain; its features declare what that
thing holds, owns, links to, computes, or can do. This page is the authority for
the forms a class and its features may take.

## Syntax

A class is a name and a body of features:

```mox,fragment
class Ticket {
    id readonly String ticketNo
    String title
    int refundCents = 0
}
```

Two modifiers may follow the class name:

```mox,fragment
class PayRun table "pay_run" extends PayRunBase {
    String period
}
```

- `extends Base` — the class inherits `Base`'s features.
- `table "name"` — an explicit storage name for consumers that need one.

### Feature forms

| Form | Kind | Stores a value |
|---|---|---|
| `String title` | attribute | yes |
| `int refundCents = 0` | attribute with a default | yes |
| `String[] tags` | many-valued attribute | yes |
| `int stock { minimum 0 }` | attribute with constraints | yes |
| `contains OrderLine[] lines opposite order` | containment (owner side) | yes, inline |
| `refers Customer[0..1] customer opposite requests` | cross-reference | a link by identity |
| `container Order order opposite lines` | containment back-pointer | no |
| `derived boolean refundable { expr { … } }` | computed feature | no |
| `op boolean approve(int cents) { expr { … } }` | operation | not a feature |

`id` and `readonly` may prefix an attribute: `id readonly String ticketNo`.
`id` marks the feature as the class's identity; `readonly` suppresses mutation.

## Semantics

| Kind | Meaning | Default cardinality |
|---|---|---|
| attribute | A stored value of a primitive, enum, datatype, or vocabulary type. | `1..1` |
| `contains` | The class owns the target objects; they are stored and serialized as part of it. | `0..*` |
| `refers` | The class links to independent objects by identity. | `0..*` |
| `container` | The child's half of a containment; always single-valued. | `0..1` |
| `derived` | A value computed from other features; it cannot be set. | `0..1` |
| `op` | An operation with parameters and a body that states its rule. | not applicable |

- **Multiplicity.** `[]` is shorthand for `[0..*]`. `[0..1]` is optional and
  single, `[1..1]` is required and single, `[0..*]` is a collection.
- **Opposites are mutual and checked.** A `contains` pairs with a `container`;
  a `refers` pairs with another `refers`. When both sides name `opposite`, the
  model verifies that they agree.
- **Containment is ownership.** Removing the owner removes the children; a child
  has at most one container. Cross-references do not have that consequence: the
  target exists independently.
- **Identity.** A class with an `id` feature distinguishes instances by that
  value even when every other feature is equal. A class without one is defined
  by its stored features.
- **Defaults.** A default is a string, integer, or boolean literal, or — for an
  enum-typed feature only — an enum literal name: `TicketState state = Open`.
  `date` features take no default.
- **Collections.** A many-valued feature is a list by default. `{ unique }`
  rejects duplicates but keeps order; `{ set }` is unordered and duplicate-free
  (`set` implies `unique`).
- **Inheritance.** `extends` adds the base class's features to the subclass.
  The base's own declarations are unchanged.

## Constraints

Value bounds are legal on attributes only; collection constraints are legal on
many-valued features only.

| Constraint | Admitted by | Effect |
|---|---|---|
| `pattern "…"` | string-typed attributes | The value matches the pattern. |
| `minLength n`, `maxLength n` | string-typed attributes | The value's length is bounded. |
| `minimum n`, `maximum n` | numeric-typed attributes | The value's magnitude is bounded. |
| `unique` | many-valued attributes, `contains`, `refers`, `derived` | Duplicate elements are rejected; order is preserved. |
| `set` | the same many-valued features | The collection is unordered and duplicate-free. |

- A datatype-typed attribute accepts the string family (its platform type is
  opaque) and rejects numeric bounds.
- An enum-typed attribute accepts both families: the string family bounds
  literal names, the numeric family bounds literal values.
- Class and interface types take no value bounds.
- `minimum` must not exceed `maximum`, and `minLength` must not exceed
  `maxLength`.
- `unique` and `set` are rejected on single-valued features. Value bounds are
  rejected on `contains`, `refers`, and `derived`.
- Two declarations in one package whose names reduce to the same module
  identifier are rejected (E0300); two classes that reduce to the same storage
  name are rejected (E0301). See **Diagnostics**.

## Examples

A warehouse that owns its bins, each bin owning its items:

```mox
package nz.example.inventory

enum ItemState {
    InStock as "in_stock" = 0
    Discontinued as "discontinued" = 1
}

class Warehouse {
    id readonly String warehouseId
    String city
    contains Bin[] bins opposite warehouse
}

class Bin {
    String code { pattern "[A-Z]-[0-9]{2}" }
    int capacity { minimum 0 maximum 500 }
    contains Item[] items opposite bin
    container Warehouse warehouse opposite bins
}

class Item {
    readonly String sku
    String name
    ItemState state = InStock
    derived boolean orderable { expr { state == ItemState.InStock } }
    op boolean reserve(int quantity) { expr { quantity > 0 } }
    container Bin bin opposite items
}
```

`Warehouse` owns its `Bin`s and each `Bin` owns its `Item`s; the `container`
features are the children's side of the same relationships. `Item` has no `id`:
a contained item is defined entirely by its data.

## Diagnostics

| Code | Meaning |
|---|---|
| `E0300` | Two class or enum names in one package reduce to the same module name. The name is reduced by removing one trailing `Type` suffix and converting to snake_case, so `PayRun` and `PayRunType` both become `pay_run`. Contained value objects are exempt: they share their owner's module by design. |
| `E0301` | Two classes reduce to the same effective storage name — a declared `table "…"` override, or the derived default. Give one class a distinct `table` name, or rename it. |

Both diagnostics name the later declaration and the declaration it collides
with.

## Compatibility

- A declared `table "…"` is carried in the emitted model; without one, consumers
  derive the storage name from the class name by the same reduction E0300 uses
  (drop one trailing `Type`, then snake_case).
- Renaming a class changes the name consumers key on. Treat a rename as a new
  identity and plan it like a schema migration.
- The emitted model carries a format version. Adding declarations and features
  does not change the version; consumers should ignore members they do not
  know. See [the versioning policy](../../../VERSIONING.md) for what 0.x
  promises.
