---
title: Choose a collection
description: Decide between a list, a unique list, and a set — and say where order is a domain fact rather than a storage accident.
type: how-to
language: mox
section: how-to
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md, docs/guide/mox/how-to/model-a-relationship.md]
related: [docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Choose a collection

Use this page when a feature holds several values and the difference between an
ordered list and an unordered set matters to the domain.

## Problem

`String[] tags` says there are several tags, but not whether duplicates are
allowed or whether their order carries meaning. Consumers then guess: one
renders tags in storage order, another sorts them, and "the same" collection
compares differently in two places.

## Prerequisites

- A domain model with at least one class.
- The [relationship how-to](model-a-relationship.md) read once; collections use
  the same multiplicity syntax.

## Solution

**1. Start with a list.** A many-valued feature with no collection constraint is
an ordered list: declaration/insertion order is preserved, duplicates are
allowed. Use it when the sequence is a domain fact — steps, tracks, lines.

```mox,fragment
contains Track[] tracks opposite playlist
```

**2. Reject duplicates while keeping order with `unique`.**

```mox,fragment
String[] tags { unique }
```

The order is still real; the collection merely refuses a repeated element.

**3. Use `set` when order carries no meaning.**

```mox,fragment
String[] categories { set }
```

A set is unordered and duplicate-free; `set` implies `unique`, so writing both
is redundant. Equality is order-insensitive by construction.

**4. Model a user-controlled order explicitly.** If people can reorder the
values, that is a domain fact: add an ordinal feature and keep the list.

```mox,fragment
int position
```

The ordinal is stored data; the list's order stays insertion order.

**5. Check the model.**

```sh
pherolang check catalog.mox
```

`unique` or `set` on a single-valued feature is reported, as is a constraint
that does not belong to the feature's kind.

## Example

```mox
package nz.example.catalog

class Playlist {
    id readonly String playlistId
    String name
    contains Track[] tracks opposite playlist
    String[] tags { set }
    String[] labels { unique }
}

class Track {
    String title
    container Playlist playlist opposite tracks
}
```

`tracks` is an ordered list: the playlist's sequence is the point. `tags` is a
set: membership matters, order does not. `labels` is an ordered distinct list:
the first label is the primary one, but a duplicate is a mistake.

## Related reference

- [Classes and features](../reference/classes.md) — the constraint table and
  the multiplicity shorthand.
- [Model a relationship](model-a-relationship.md) — ownership and links.
- [The .mox language reference](../../../../LANGUAGE.md#collections-lists-and-sets)
  — the deterministic serialization rule and how consumers compare collections.
