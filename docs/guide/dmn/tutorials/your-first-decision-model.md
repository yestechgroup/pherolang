---
title: Tutorial — your first decision model
description: "Build a refund decision model over an existing domain: typed inputs, an expression decision, a table-driven decision, and a decision service."
type: tutorial
language: dmn
section: tutorials
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md, docs/guide/mox/overview.md]
related: [docs/guide/dmn/how-to/write-a-decision-table.md, docs/guide/dmn/reference/decision-models.md]
lastReviewed: 2026-11-21
---

# Tutorial — your first decision model

This tutorial builds a refund decision model over the support-desk domain: an
eligibility decision expressed directly, an approval ladder implemented as an
external decision table, and a decision service that exposes both.

## What you'll learn

- Declare a decision model with typed inputs from a domain model.
- Write an expression decision and its information requirement.
- Back a decision with an external markdown decision table.
- Expose decisions through a decision service.
- Map a decision outcome to a declared event.
- Check the model and read its diagnostics.

## Prerequisites

- Phero Lang installed, with `pherolang --version` working.
- The repository checked out. This tutorial uses `examples/support`.
- The [decision model overview](../overview.md) read once.

## What we're building

One file, `refunds.dmn`, next to copies of the support domain, its event
contract, and one table. When it is done, `pherolang check refunds.dmn` prints
`OK refunds.dmn`.

Create a working directory and copy the pieces:

```sh
mkdir refunds
cd refunds
mkdir tables
cp <path-to-repository>/examples/support/support.mox .
cp <path-to-repository>/examples/support/support.evt .
cp <path-to-repository>/examples/support/tables/refund-authority.md tables/
```

## Step 1 — Declare the model and its inputs

```dmn,fixture=examples/support
import "support.mox";

decision_model RefundDecisions {
    namespace nz.example.support.decisions;

    inputs {
        ticket: Ticket;
        requested_cents: int;
    }
}
```

`import` resolves relative to the model file. `inputs` names the data the
decisions will consume; `Ticket` resolves against the domain, so a typo is an
unknown-type error rather than a silent string.

```sh
pherolang check refunds.dmn
```

You should see `OK refunds.dmn`. A model with inputs and no decisions is valid:
the graph may carry data beyond one evaluation's reach.

## Step 2 — Add an expression decision

```dmn,fixture=examples/support
import "support.mox";

decision_model RefundDecisions {
    namespace nz.example.support.decisions;

    inputs {
        ticket: Ticket;
        requested_cents: int;
    }

    decisions {
        decision RefundEligibility {
            output boolean;
            logic expression "ticket.needsRefund and requested_cents > 0"
        }
    }

    requirements {
        information {
            RefundEligibility requires ticket;
            RefundEligibility requires requested_cents;
        }
    }
}
```

A decision has an output contract (`output boolean`) and one logic clause. The
information requirements name what it reads — they are what turns the
declarations into a graph, and they must name declared inputs or decisions.

```sh
pherolang check refunds.dmn
```

## Step 3 — Add a table-driven decision

```dmn,fixture=examples/support
import "support.mox";

decision_model RefundDecisions {
    namespace nz.example.support.decisions;

    inputs {
        requested_cents: int;
    }

    decisions {
        decision RefundAuthority {
            outputs {
                requires_approval: boolean;
                approver_role: string;
                maximum_amount: int;
            }
            logic table "./tables/refund-authority.md"
        }
    }

    requirements {
        information {
            RefundAuthority requires requested_cents;
        }
    }
}
```

The output names match the table's output columns exactly. The decision's
information-required inputs (`requested_cents`) must be exactly the table's
input columns — the compiler checks both directions, so the signature and the
rules cannot drift.

```sh
pherolang check refunds.dmn
```

## Step 4 — Expose a decision service

```dmn,fixture=examples/support
import "support.mox";

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

`AssessRefund` is the invocation boundary: the caller supplies the two inputs
and receives both decision results. `encapsulates` names the elements inside the
service — here, the two decisions. The `version` line is the compatibility
contract callers can pin.

```sh
pherolang check refunds.dmn
```

## Step 5 — Emit the outcome event

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

The event contract is imported by the name `.dmn` matches exactly, and
`RefundApproved` must be declared there. The `when` condition names the output
that decides whether the event fires.

```sh
pherolang check refunds.dmn
```

## Complete example

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

## Expected result

```sh
$ pherolang check refunds.dmn
OK refunds.dmn
```

The command exits with status 0. The model declares two decisions, one table
contract, one event outcome, and one service; every type, requirement, column,
and event name resolves.

## Next steps

- [Write a decision table](../how-to/write-a-decision-table.md) explains the
  markdown contract in detail.
- [Connect decisions to the application](../how-to/connect-decisions-to-the-application.md)
  invokes the service from `.ddd`, binds it in `.ifml`, and covers version pins.
- [The decision-requirements graph](../concepts/decision-requirements-graph.md)
  explains how knowledge and authority requirements fit.
- The [support-desk decision model](../../../../examples/support/support.dmn) is
  the full example, including a business knowledge model and views.
