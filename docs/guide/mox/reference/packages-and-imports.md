---
title: Packages and imports
description: Name a model's namespace, refer across packages, compile several files as one model, and import JSON Schema or Rune types.
type: reference
language: mox
section: reference
order: 30
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md]
related: [docs/guide/mox/reference/enums-datatypes-and-vocabularies.md]
lastReviewed: 2026-11-21
---

# Packages and imports

Every `.mox` file declares one package. A model may span several files, and a
package may import JSON Schema or Rune types into its namespace.

## Syntax

```text
package <qualified.name>

import schema "<path>.json" ("as" <Name>)?
import sigil "<path>.rosetta"

class <Name> { … }
```

```mox,fragment
import schema "schemas/todo_item.json" as TodoItem
import sigil "models/pricing.rosetta"
```

## Semantics

- **One package per file**, named by its `package` declaration. Compiling
  several files produces one model whose packages follow the input order.
- **Bare names must be unique across the whole compilation.** If several
  packages declare `Ticket`, `refers Ticket` is ambiguous; qualify it as
  `p.Ticket`.
- **A qualified name** must match exactly one package.
- **Cross-package references are allowed** for `refers`, `extends`, and
  attribute types. `contains`/`container` targets and `opposite` pairings must
  live in the declaring class's package.
- **An imported schema is a nominal, feature-less class** in v1. The import name
  is the `as` alias, else the file stem; the path resolves relative to the
  declaring file. The content is validated as JSON, but no features are derived
  from it yet.
- **A sigil import** lowers Rune (`sigil`) declarations into synthetic packages
  whose functions and types are usable from the model.
- **`pherolang fmt` hoists the import declarations** into a section directly
  after `package`.

## Constraints

- A duplicate declaration in one package is an error.
- A bare reference that matches several packages is an error whose help lists
  the matching packages; qualify it.
- A `contains`/`container` pair or an `opposite` across packages is an error.
- A schema import path must resolve (relative to the file) and its content must
  parse as JSON; a missing import is an error, never an empty class.
- A stem-derived import name must be a valid identifier; otherwise write an
  explicit alias.
- A sigil import must be provided by the host, like a schema import.

## Examples

```mox
package nz.example.library

enum BookCategory {
    Mystery as "M" = 0
    ScienceFiction as "S" = 1
}

class Library {
    id readonly String libraryId
    contains Book[] books opposite library
}

class Book {
    String title
    BookCategory category
    container Library library opposite books
    refers Author[] authors opposite books
}

class Author {
    String name
    refers Book[] books opposite authors
}
```

One package, three declarations, and two relationship pairs. Adding a second
file with a different `package` and compiling both files together makes their
types referable across the boundary; `contains` and `opposite` stay inside a
package.

## Diagnostics

| Situation | Report |
|---|---|
| A type name matches no declaration | `unknown type '<type>' on <what> '<name>'` |
| A bare name matches several packages | `ambiguous type 'X'; qualify as 'p.X'`, with the matching packages in the help |
| A relationship crosses a package | An error naming the containment or opposite rule |
| An import is missing or unparsable | An error naming the resolved path (a schema must parse as JSON) |
| Two declarations reduce to the same module name | `E0300` |
| Two classes reduce to the same storage name | `E0301` |

## Compatibility

- A multi-file compile emits **one** model; consumers see packages in input
  order.
- Imported schema types are nominal in v1: an empty class in the model, with the
  import itself recorded.
- Sigil lowering turns Rune namespaces into synthetic packages; their functions
  are carried as generated targets.
- The package name is part of every class's identity for consumers keyed by
  `(package, class, feature)`.
