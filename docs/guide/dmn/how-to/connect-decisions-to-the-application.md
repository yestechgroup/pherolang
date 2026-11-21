---
title: Connect decisions to the application
description: Invoke a decision service from an application design, bind decision data in a flow, and pin the model version callers rely on.
type: how-to
language: dmn
section: how-to
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md, docs/guide/dmn/tutorials/your-first-decision-model.md]
related: [docs/guide/dmn/reference/decision-services.md, docs/guide/ddd/reference/services-and-operations.md]
lastReviewed: 2026-11-21
---

# Connect decisions to the application

Use this page when a decision service must be invoked by an application
operation, shown on a screen, or tied to the events it emits.

## Problem

A decision model that nothing calls is documentation. The application needs to
invoke the service with the right inputs, the interaction layer needs to show
its outputs, and the contract must not silently change when the decision model
is edited.

## Prerequisites

- A decision model with a decision service and a `version` line.
- An application design (`.ddd`) and/or an interaction flow (`.ifml`).
- The [tutorial](../tutorials/your-first-decision-model.md) completed.

## Solution

**1. Invoke the service from an application operation.** The `invokes` clause
names the decision service; the service's inputs are the operation's
responsibility.

```ddd,fragment
boolean assess(String ticketNo, int cents) invokes AssessRefund;
```

**2. Inject the decision service when the operation needs it as a dependency.**

```ddd,fragment
inject decision AssessRefund version compatible;
```

**3. Pin the version the caller relies on.** No pin resolves by name; `version
compatible` selects the highest version in the highest major; an exact
`version "X.Y.Z"` selects that generation.

```ddd,fragment
boolean approve() invokes AssessRefund version compatible;
```

**4. Bind decision data in a flow.** A component's `data:` may name a decision
service when no domain class matches; the flow imports the decision model.

```ifml,fragment
import "support.dmn";

component "assessment" {
    type: details;
    data: AssessRefund;
}
```

**5. Map the outcome to an event.** The decision's `emits` block names events
the imported `.evt` contract declares; subscribers use the event, not the
decision.

```dmn,fragment
emits {
    RefundApproved when "requires_approval";
}
```

**6. Check the application, not just the model.**

```sh
pherolang check .
```

The project check gathers the application's decision models, so the `invokes`
and `inject decision` links are validated. A single-file check of the `.ddd`
without the models skips them — it cannot prove the link.

## Example

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

`AssessRefund` is what `.ddd` invokes and what `.ifml` can bind; `RefundApproved`
is what `.evt` subscribers receive. The version line is what callers pin against.

## Related reference

- [Decision services](../reference/decision-services.md) — the contract and the
  version rules.
- [Services and operations](../../ddd/reference/services-and-operations.md) —
  the `.ddd` clause order (`capability? invokes? emits? …`).
- [Views and components](../../ifml/reference/views-and-components.md) — the
  component `data:` binding.
- [The .dmn language reference](../../../../DMN.md#cross-surface-integration) —
  the authoritative cross-surface contract.
