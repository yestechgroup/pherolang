---
title: Decision models (.dmn)
description: Describe business decisions as a graph of inputs, knowledge, and rules — typed by the domain model and invoked through a decision service.
type: overview
language: dmn
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/dmn/concepts/omg-dmn-conformance.md, docs/guide/dmn/reference/decision-models.md]
lastReviewed: 2026-11-21
---

# Decision models (.dmn)

A `.dmn` file describes the decisions an application makes: the inputs they
need, the knowledge they answer to, the rules that turn inputs into outcomes,
and the service that exposes the result to the rest of the system.

## What is it?

A decision model is a decision-requirements graph written as declarations. It
names **inputs** (typed values the decision needs), **decisions** (each with an
output contract and a rule), and optionally **knowledge** — sources that govern
a decision and reusable business knowledge models it invokes. A **decision
service** wraps the decisions a caller actually uses into one invocation
contract.

The rules themselves can live in an external markdown decision table. That
split is deliberate: the `.dmn` declares the signature, the table implements the
rules, and the compiler holds the two together — a table whose input or output
columns do not match the decision's contract is an error.

Types come from the domain model. A decision input can be `Ticket` or `User`, not
just a primitive, so the decision is typed against the same model as the rest of
the application.

## What problems does it solve?

Business rules usually live in code paths that cannot be reviewed by the people
who own them, or in documents that no test executes. Neither can answer "which
decisions does this outcome depend on?".

A decision model makes the rules a checked part of the application: the graph
is explicit, the decision table is a data file a domain expert can read, and every input, output, and
knowledge link is resolved at compile time. Decision tables stop drifting from
the code that calls them, because the code calls a declared service with a
declared contract.

## Key capabilities

- **Typed inputs.** Inputs resolve against the imported `.mox` domain — classes,
  enums, datatypes, and primitives.
- **Decision contracts.** A decision declares one output (`output T;`) or a
  named output set (`outputs { … }`).
- **Three logic kinds.** A rule is an expression, a table, or an invocation of a
  business knowledge model.
- **External decision tables.** Rules live in markdown; the compiler checks the
  table's columns against the decision's contract.
- **Knowledge and authority.** Business knowledge models are reusable functions;
  sources record the authorities a decision answers to.
- **Decision services.** One invocation contract for the decisions a caller
  needs, with versioned compatibility.
- **Outcome events.** A decision can name the declared events its outcome emits.
- **Views.** Presentation intent for the graph — what to show and in which
  direction — without diagram interchange.

## A simple example

A refund-eligibility decision and a table-driven approval ladder over the
support domain, exposed through one service:

```dmn,fixture=examples/support
import "support.mox";
import "support.evt";

decision_model RefundDecisions {
    namespace nz.example.support.decisions;
    version "1.0.0";

    inputs {
        ticket: Ticket;
        requested_cents: int;
    }

    decisions {
        decision RefundEligibility {
            output boolean;
            logic expression "ticket.needsRefund and requested_cents > 0"
        }
        decision RefundAuthority {
            outputs {
                requires_approval: boolean;
                approver_role: string;
                maximum_amount: int;
            }
            logic table "./tables/refund-authority.md"
            emits {
                RefundApproved when "requires_approval";
            }
        }
    }

    requirements {
        information {
            RefundEligibility requires ticket;
            RefundEligibility requires requested_cents;
            RefundAuthority requires requested_cents;
        }
    }

    services {
        service AssessRefund {
            inputs { ticket requested_cents }
            outputs { RefundEligibility RefundAuthority }
            encapsulates { RefundEligibility RefundAuthority }
        }
    }
}
```

`RefundEligibility` is one expression over the ticket. `RefundAuthority` is
table-driven: its rules live in `./tables/refund-authority.md`, and its output
set matches the table's output columns. Both decisions require the inputs they
read, and `AssessRefund` is the contract a caller invokes.

## How it fits into Phero Lang

- `.mox` supplies the types every input, output, and parameter resolves against.
- `.ddd` **invokes** a decision service from an application operation, and may
  inject the service as a dependency.
- `.ifml` may bind a component's `data:` to a decision service, so a screen shows
  a decision's outputs.
- `.evt` declares the events a decision's `emits` block names.
- `.cmmn` decision tasks invoke decision services the same way operations do.
- `.deploy` projects the application, including its decision models, onto
  runtime targets.

A lending application, for example, models the customer and loan in `.mox`,
the credit decision in `.dmn`, and the advisor's screens in `.ifml` — all over
one typed model.

## Getting started

- [Tutorial — your first decision
  model](../tutorials/your-first-decision-model.md) builds the example above
  step by step.
- [Write a decision table](../how-to/write-a-decision-table.md) covers the
  external table contract.
- [Connect decisions to the application](../how-to/connect-decisions-to-the-application.md)
  wires `.ddd`, `.ifml`, and `.evt`.
- [Decision services](../reference/decision-services.md) covers invocation and
  version pins.

## Further reading

- [The decision-requirements graph](../concepts/decision-requirements-graph.md)
  explains how the parts compose.
- [OMG DMN conformance](../concepts/omg-dmn-conformance.md) states exactly how
  the language relates to the standard.
- [Decision models](../reference/decision-models.md) is the syntax reference.
- [The .dmn language reference](../../../DMN.md) is the complete engineering
  reference, including the table engine seam and validation rules.
