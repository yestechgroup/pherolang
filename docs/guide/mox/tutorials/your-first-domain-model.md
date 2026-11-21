---
title: Tutorial — your first domain model
description: "Build a small refund desk from an empty directory: packages, classes, an enum, a relationship, a derived value, and an operation."
type: tutorial
language: mox
section: tutorials
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/mox/overview.md]
related: [docs/guide/mox/how-to/add-an-enum.md, docs/guide/mox/reference/classes.md]
lastReviewed: 2026-11-21
---

# Tutorial — your first domain model

By the end of this tutorial you will have a `domain.mox` file that describes a
refund desk — customers, their refund requests, and the rules around them — and
you will have run Phero Lang's checks against it. Every step ends with a
command you can run and a result you can compare.

## What you'll learn

- Create a `.mox` file with a package and a class.
- Give a class an identity and connect two classes with a relationship.
- Model a lifecycle with an enum and a default value.
- Compute a value and state an operation's rule with an expression.
- Check the model and read the JSON it produces.

## Prerequisites

- Phero Lang installed, with `pherolang --version` working.
- A terminal and a text editor.
- The [domain models overview](../overview.md) read once. You do not need to
  memorize it.

## What we're building

One file, `refund-desk/domain.mox`, with three declarations: an enum, a
`Customer`, and a `RefundRequest` that refers to its customer. When it is done,
`pherolang check domain.mox` prints `OK domain.mox`.

## Step 1 — Create the package and a class

Create the directory and the file:

```sh
mkdir refund-desk
cd refund-desk
```

Write `domain.mox`:

```mox
package nz.example.refunds

class Customer {
    id readonly String customerId
    String name
}
```

`package` names the namespace for everything in the file. `Customer` has two
features: `customerId`, marked `id` because two customers with the same name are
still two customers, and a `name`.

Check it:

```sh
pherolang check domain.mox
```

You should see `OK domain.mox`.

## Step 2 — Add the refund request and its customer

A refund request belongs to one customer. Add a second class that points at
`Customer`:

```mox
package nz.example.refunds

class Customer {
    id readonly String customerId
    String name
}

class RefundRequest {
    id readonly String requestId
    int amountCents
    String reason
    refers Customer[1..1] customer
}
```

`refers` is a link to another object, not ownership: the customer exists
independently and can have many requests. `[1..1]` says every request has
exactly one customer.

Check again:

```sh
pherolang check domain.mox
```

Still `OK domain.mox`.

## Step 3 — Give the request a lifecycle

A request moves through states. Model the states as an enum and add a feature
that holds the current one:

```mox
package nz.example.refunds

enum RefundState {
    Requested as "requested" = 0
    Approved as "approved" = 1
    Rejected as "rejected" = 2
}

class Customer {
    id readonly String customerId
    String name
}

class RefundRequest {
    id readonly String requestId
    int amountCents
    String reason
    RefundState state = Requested
    refers Customer[1..1] customer
}
```

Each enum literal has a stable integer value and a name. The name (`Requested`)
is what canonical JSON and the generated schema carry; the optional
`as "requested"` label is display metadata. The `= Requested` default means a
new request starts in the requested state.

Run the check once more:

```sh
pherolang check domain.mox
```

## Step 4 — Add a computed value and an operation

Two more declarations make the model useful. `needsApproval` is computed from
the stored amount; `approve` states the rule that a full approval matches the
requested amount exactly:

```mox
package nz.example.refunds

enum RefundState {
    Requested as "requested" = 0
    Approved as "approved" = 1
    Rejected as "rejected" = 2
}

class Customer {
    id readonly String customerId
    String name
}

class RefundRequest {
    id readonly String requestId
    int amountCents
    String reason
    RefundState state = Requested
    refers Customer[1..1] customer
    derived boolean needsApproval { expr { amountCents > 5000 } }
    op boolean approve(int cents) { expr { cents == amountCents } }
}
```

`derived` features are not stored: `needsApproval` is computed whenever it is
read. The operation body is an expression in braces; `cents` is its parameter
and `amountCents` is the object's own feature.

```sh
pherolang check domain.mox
```

Still `OK domain.mox`.

## Step 5 — Produce the model as JSON

The same file can be emitted as JSON for tools that consume it:

```sh
pherolang ir domain.mox -o target/domain.json
```

Open `target/domain.json`. It begins with the model's format version and then a
`packages` array; your package `nz.example.refunds` contains the three
declarations you wrote, in the order you wrote them. Nothing in the file needs
to be handwritten again by another tool.

## Complete example

```mox
package nz.example.refunds

enum RefundState {
    Requested as "requested" = 0
    Approved as "approved" = 1
    Rejected as "rejected" = 2
}

class Customer {
    id readonly String customerId
    String name
}

class RefundRequest {
    id readonly String requestId
    int amountCents
    String reason
    RefundState state = Requested
    refers Customer[1..1] customer
    derived boolean needsApproval { expr { amountCents > 5000 } }
    op boolean approve(int cents) { expr { cents == amountCents } }
}
```

## Expected result

```sh
$ pherolang check domain.mox
OK domain.mox
```

The command exits with status 0. `target/domain.json` exists and contains a
`packages` array with one package, `nz.example.refunds`.

## Next steps

- [Add an enum to a domain model](../how-to/add-an-enum.md) covers the enum
  syntax in isolation, including when to use labels.
- [Classes and features](../reference/classes.md) is the reference for every
  form this tutorial used — and the ones it did not.
- [Modelling, not coding](../concepts/modelling-not-coding.md) explains why the
  model is written before — and separately from — the code that consumes it.
- When the domain is stable, the `.ddd` guide shows how to describe the
  application built on top of it.
