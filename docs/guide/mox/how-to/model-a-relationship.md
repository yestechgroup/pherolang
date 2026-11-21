---
title: Model a relationship
description: Choose between ownership and a link, pair the two sides with opposite, and set the cardinality that matches the domain.
type: how-to
language: mox
section: how-to
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md, docs/guide/mox/tutorials/your-first-domain-model.md]
related: [docs/guide/mox/how-to/choose-a-collection.md, docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Model a relationship

Use this page when two classes are connected and you need to decide what the
connection means: does one object own the other, or does it merely link to it?

## Problem

A relationship written as a plain attribute — `String libraryId` — loses the
connection: nothing checks that the id names a library, nothing pairs the two
sides, and a consumer has to reconstruct the join. The model needs to say
whether the target is part of the object or independent of it.

## Prerequisites

- The domain model from the [tutorial](../tutorials/your-first-domain-model.md)
  or an equivalent file.
- Both classes declared in the same package.
- The [overview](../overview.md) read once.

## Solution

**1. Decide what the relationship means.**

| The child… | Use |
|---|---|
| cannot exist without the parent, and is stored with it | `contains` on the parent, `container` on the child |
| exists independently and is referred to by identity | `refers` on both sides |

**2. Declare the owning side.**

```mox,fragment
class Library {
    contains Book[] books opposite library
}
```

**3. Declare the child's side with the same pair.** A `contains` pairs with a
`container`; a `refers` pairs with another `refers`. When both sides name
`opposite`, the model verifies that they agree.

```mox,fragment
class Book {
    container Library library opposite books
}
```

**4. Declare a link when the target is independent.**

```mox,fragment
class Book {
    refers Writer[] authors opposite books
}
```

**5. Set the cardinality on the owning side.** `[]` is `[0..*]`; `[0..1]` is
optional and single; `[1..1]` is required and single. A `container` never
carries a multiplicity.

**6. Check the model.**

```sh
pherolang check library.mox
```

An unpaired `opposite`, a multiplicity on a `container`, or a relationship to
an unknown class is reported at the declaration.

## Example

```mox
package nz.example.library

class Library {
    id readonly String libraryId
    String name
    contains Book[] books opposite library
}

class Book {
    String title
    container Library library opposite books
    refers Writer[] authors opposite books
}

class Writer {
    String name
    refers Book[] books opposite authors
}
```

The library owns its books: a book has one library, and removing the library
removes its books. Writers are independent: books and writers link to each
other, and deleting a writer leaves the books in place.

## Related reference

- [Classes and features](../reference/classes.md) — the exact syntax for
  `contains`, `refers`, `container`, and `opposite`.
- [Choose a collection](choose-a-collection.md) — list, unique list, and set.
- [The .mox language reference](../../../../LANGUAGE.md#collections-lists-and-sets)
  — the engineering contract for relationship serialization.
