---
title: Decision services
description: The invocation contract a caller binds to — inputs, outputs, encapsulation, and version pins.
type: reference
language: dmn
section: reference
order: 20
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md]
related: [docs/guide/dmn/how-to/connect-decisions-to-the-application.md, docs/guide/dmn/reference/decision-models.md]
lastReviewed: 2026-11-21
---

# Decision services

A decision service is the boundary a caller invokes: the model's inputs it
supplies, the decisions it receives, and everything the boundary encapsulates.

## Syntax

```text
service <Name> {
    inputs { <input> … }
    outputs { <decision> … }
    encapsulates { <element> … }
}
```

## Semantics

- **`inputs`** names the model's input data the caller must supply.
- **`outputs`** names the decisions whose results the caller receives.
- **`encapsulates`** names every element inside the boundary: decisions,
  business knowledge models, and knowledge sources it uses.
- **A service carries no logic and no requirements of its own.** It is a naming
  and invocation boundary; the graph does the work.
- **Naming convention.** Services are PascalCase; model elements are snake_case.
- **The model's `version` is the service's generation.** The authoring rule is
  same-major compatibility: a breaking change to a service's inputs or outputs
  bumps the major.

## Constraints

- Every name in all three lists must resolve (`E0206`).
- A decision may appear in more than one service; the boundary is a view of the
  graph, not a copy.
- An input no decision requires may still be a service input — a service is a
  caller contract, and the caller may supply data the graph currently ignores.
- A service output must be a decision; a BKM or source is internal.
- Version pins on the consumer side resolve as: no pin (by name), `version
  compatible` (highest version in the highest major present), or an exact
  `version "X.Y.Z"`.

## Examples

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

`AssessRefund` takes both inputs and returns both decisions. A caller that only
needs eligibility could bind a narrower service without changing the graph.

## Diagnostics

| Code | Meaning |
|---|---|
| `E0206` | A service endpoint names an element the model does not declare. |
| `E0250` | A `.ddd`/`.cmmn` operation invokes an unknown decision service. |
| `E0251` | An `inject decision` line names an unknown decision service. |
| `E0252` | A pinned version is not available in the provided models. |
| `E0253` | Two models declare the same service at the pinned version. |
| `E0254` | No provided model can satisfy `version compatible`. |

## Compatibility

- Consumers pin the model's `version`; the compiler does not diff signatures
  across versions, so the same-major rule is authored discipline.
- `version compatible` resolves to the highest declared version in the highest
  major present in the decision set — a major bump is a breaking change by
  agreement.
- A service's contract is the inputs/outputs it declares; everything else in the
  model is internal to callers.
