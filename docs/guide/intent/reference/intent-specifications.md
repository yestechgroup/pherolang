---
title: Intent specifications
description: The intent declaration with its stable ids, quoted statements, element and check link kinds, and the project-wide coverage rules.
type: reference
language: intent
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/intent/overview.md]
related: [docs/guide/intent/concepts/requirements-as-data.md]
lastReviewed: 2026-11-21
---

# Intent specifications

A `.intent` file holds imports of domain models followed by `intent`
declarations. This page is the authority for the shape of those declarations,
the link kinds, and the rules the project applies when it resolves them.

## Syntax

```text
import "<domain>.mox";

intent <STABLE-ID> "<statement>" {
    element <kind>:<name>;
    check <kind>:<name>;
}
```

Zero or more imports come first. Each `intent` declaration has a stable id, a
quoted statement, and a body of link lines; a link's closing `;` is canonical
but may be omitted.

A body with one link of each slot:

```intent,fragment
intent REFUND-002 "A raised refund is always assessed before money moves." {
    element op:TicketService.raiseRefund;
    element event:RefundRequested;
    check decision:RefundAssessment;
}
```

The element slot accepts six kinds, the check slot two:

| Slot | Kind | A name resolves to |
|---|---|---|
| `element` | `class` | A class of the imported domain models. |
| `element` | `capability` | A capability declared by the application's actor policies. |
| `element` | `event` | An event declared by the application's event contracts. |
| `element` | `view` | A view declared by the application's interaction flows. |
| `element` | `command` | A command declared by the application's interaction flows. |
| `element` | `op` | An operation of a service in the application designs. |
| `check` | `invariant` | A class in the application designs that carries at least one invariant. |
| `check` | `decision` | A decision declared by the application's decision models. |

A stable id is an uppercase lead segment followed by hyphen-separated segments,
at least one of which is all digits: `REFUND-001`, `REFUND-DESK-007`. The id is
read from the source exactly as written, so leading zeros are preserved.

## Semantics

- **Imports come first.** An `import` after the first `intent` declaration is a
  syntax error. An import path is resolved in the project first and then
  relative to the file that writes it.
- **Imports name domain models only.** The import exists so the file is grouped
  with the right application. It brings no link target into scope; a link is a
  name resolved across the application, never an import.
- **The id is the requirement's identity.** The written text is the id, so
  `REFUND-001` and `REFUND-1` are distinct. Reordering declarations or renaming
  a linked element does not change an id.
- **The statement is kept as written.** It is the requirement in the author's
  words, apart from surrounding whitespace. Nothing interprets or evaluates it;
  it must be non-empty after trimming.
- **An element link says what expresses the requirement.** A `class` is matched
  by bare name or by `package.Class`. An `op` is matched by bare name or by
  `Service.operation`. The other element kinds are matched by the declared name
  alone.
- **A check link says what verifies the requirement.** An `invariant` names a
  class in the application designs that carries at least one invariant; a
  `decision` names a decision of a decision model. Neither has a qualified
  spelling.
- **One link per target per intent.** The same kind and name may not appear
  twice in one intent's links.
- **Kinds are checked when the file is read; names are checked by the project.**
  Reading a file validates the id, the statement, and each link's kind against
  its slot. Resolving a name needs the whole application, so it happens in the
  project's coverage pass, which `pherolang check` runs after the per-file
  checks; the [engineering reference](../../../INTENTS.md) documents that pass.
- **Coverage is reported, never guessed.** An intent is expected to resolve at
  least one element and at least one check. A missing slot is a warning, because
  an uncovered requirement is a claim nobody can honor and that state should be
  visible. A name that resolves to nothing is an error; a bare name that matches
  several targets is an error, because the pass refuses to pick a target for
  you.
- **Ids are unique across the application.** Two intents in one file may not
  share an id, and two intent files of one application may not either.
- **The semantic graph carries the traceability.** Each intent is a node. Every
  resolved element link adds an `expresses` edge from the intent to that
  element, and every resolved check link adds a `verifiedBy` edge. The edges
  point out of the intent, so `affected-by` on an expressing element reaches the
  requirement that names it, and `dependencies` on a requirement names what
  expresses and verifies it. A link the project cannot resolve to exactly one
  target adds no edge. The [graph reference](../../../GRAPH.md) describes the
  whole picture.

## Constraints

| Rule | What happens when it is broken |
|---|---|
| The id is the stable shape. | `E0450`; the intent is not kept. |
| Ids are unique within a file. | `E0451`. |
| The statement is non-empty after trimming. | `E0452`. |
| A kind word belongs to its slot's closed list. | `E0453`, naming the allowed kinds. |
| One intent links the same kind and name once. | `E0454`. |
| Imports precede the first intent. | A syntax error at the late import. |
| Imports name `.mox` files. | A link target cannot be imported; the import is refused. |
| Every link name resolves in the application. | `X0011` for an element, `X0012` for a check; a close spelling is offered when one exists. |
| A bare name matches one target. | `X0013`; qualify a class as `package.Class`, an operation as `Service.operation`, or rename one target. |
| Every intent should resolve an element and a check. | `X0009` and `X0010` warnings. |
| Ids are unique across the application's intent files. | `X0014`. |

An intent with an empty body is valid: it declares a requirement and claims
nothing about where that requirement lives. The project reports the two missing
slots, so the claim is visible as a gap rather than absent from the report.

A link that does not resolve leaves its slot uncovered, so an unknown name
produces its error and the corresponding missing-slot warning. Fix the name to
clear both.

## Examples

A file over the support application, with two fully linked requirements:

```intent,fixture=examples/support
import "support.mox";

intent REFUND-001 "A refund above the desk's auto-approval limit is decided by a human." {
    element class:Ticket;
    element capability:RaiseRefund;
    element capability:ApproveRefund;
    element event:RefundRequested;
    check decision:RefundAuthority;
}

intent REFUND-002 "A raised refund is always assessed before money moves." {
    element op:TicketService.raiseRefund;
    element event:RefundApproved;
    check decision:RefundAssessment;
}
```

The first intent names a class, two capabilities, an event, and a decision. The
second qualifies its operation as `TicketService.raiseRefund` and relies on two
unqualified names. The single import groups the file with the support
application; every link resolves across that application.

A requirement with no links still compiles:

```intent
intent REFUND-004 "A refund decision is recorded against the ticket that requested it." {
}
```

The project reports it as having no element and no check. That is the intended
outcome: the requirement is kept and its gap is made visible, not hidden.

## Diagnostics

| Code | Severity | Meaning |
|---|---|---|
| `E0450` | error | The id is not the stable shape: an uppercase lead segment plus at least one all-digits segment is required. |
| `E0451` | error | Two intents in one file declare the same id. |
| `E0452` | error | The statement is empty or only whitespace. |
| `E0453` | error | The kind word is not in its slot's closed list. |
| `E0454` | error | One intent links the same kind and name twice. |
| `X0009` | warning | The intent resolves no element of the application. |
| `X0010` | warning | The intent resolves no check. |
| `X0011` | error | An element link names no element of its kind; a close spelling is offered when one is known. |
| `X0012` | error | A check link names no target of its kind: no class carrying invariants, or no decision. |
| `X0013` | error | A bare link name matches several targets of its kind. |
| `X0014` | error | A stable id is declared by two intent files of one application. |

## Compatibility

- A stable id is the requirement's identity everywhere. It survives reordering,
  the renaming of a linked element, and a fresh check; two spellings that differ
  only in leading zeros are different ids.
- Link names are carried exactly as written. Nothing rewrites them, so renaming
  an element in another language requires updating the link that names it, and
  the project reports the break until you do.
- Intents keep their declaration order within a file.
- Reading one file never resolves a name. Only the project-wide coverage pass
  does.
