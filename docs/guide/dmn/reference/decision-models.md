---
title: Decision models
description: The decision_model declaration — metadata, imports, inputs, knowledge, decisions, logic kinds, requirements, and views.
type: reference
language: dmn
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md]
related: [docs/guide/dmn/reference/decision-services.md, docs/guide/dmn/concepts/decision-requirements-graph.md]
lastReviewed: 2026-11-21
---

# Decision models

A `.dmn` file holds exactly one decision model: the graph's inputs, knowledge,
decisions, requirements, and views, optionally wrapped by decision services.

## Syntax

```text
import "<domain>.mox";
import "<contracts>.evt";

decision_model <Name> {
    namespace <qualified.name>;
    id "<id>";
    version "<X.Y.Z>";
    effective "<date>";
    jurisdiction "<code>";

    inputs {
        <name>: <Type>;
    }

    knowledge {
        source <Name> {
            name "<label>";
            reference "<uri>";
        }
        model <name> {
            parameters { <name>: <Type>; }
            output <Type>;
            logic expression "<FEEL>";
        }
    }

    decisions {
        decision <name> {
            output <Type>;                  // or:
            outputs { <name>: <Type>; }
            logic expression "<FEEL>";
            logic table "<path>.md";
            logic invoke <model> { <parameter>: "<FEEL>"; }
            emits { <Event> (when "<FEEL>")?; }
        }
    }

    requirements {
        information { <Decision> requires <Input|Decision>; }
        knowledge   { <Decision> invokes <Model>; }
        authority   { <Decision> governed_by <Source>; }
    }

    views {
        view <name> {
            description "<text>";
            show { <element> … }
            direction left_to_right;
        }
    }

    services { … }                     // see Decision services
}
```

## Semantics

- **One model per file.** The `decision_model` is the file's single top-level
  declaration; imports come first.
- **Imports** resolve `.mox` domains (exact path or relative resolution) and
  `.evt` contracts (exact path as written). A `.dmn` never imports `.ddd` or
  another `.dmn`.
- **Types** in inputs, outputs, and parameters resolve against the imported
  domain union; with no imports every type must be a primitive.
- **One namespace per model.** Every element name — inputs, sources, models,
  decisions, services, views — shares it and must be unique.
- **Output contracts.** `output T;` declares a single unnamed output;
  `outputs { name: T; }` declares a named set. The table contract follows these
  names.
- **Logic kinds.** A decision has at most one logic clause of the three kinds:
  an expression, a table, or an invocation of a business knowledge model.
- **Information requirements** fix a table decision's input signature: exactly
  the table's input columns.
- **`emits`** maps an outcome to events a provided `.evt` contract declares. With
  no contract provided, the names lower as authored.
- **Views** declare presentation intent only: what to show, and a direction
  label. They are not a diagram format.

## Constraints

- Element names are unique across the model (`E0204`); metadata lines appear at
  most once (`E0214`).
- `version` is strict semver (`X.Y.Z`); a breaking service change bumps the
  major.
- Every requirement target must be declared: information (`E0201`), knowledge
  model (`E0202`), source (`E0203`).
- Information requirements must be acyclic (`E0205`).
- Unknown types are `E0200`; malformed FEEL is `E0208`.
- A table must be provided (`E0209`), parse as one table (`E0210`), and match the
  decision's inputs (`E0211`) and outputs (`E0212`).
- An `invoke` must bind the model's parameters and output (`E0213`).
- An emitted event must be declared in a provided contract (`E0215`).
- A view may show only declared elements (`E0207`).

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

## Diagnostics

| Code | Meaning |
|---|---|
| `E0200` | An input, output, or parameter type names an unknown type. |
| `E0201`–`E0203` | A requirement names an undeclared input/decision, model, or source. |
| `E0204`, `E0214` | Duplicate element name; duplicate metadata line. |
| `E0205` | Information requirements form a cycle. |
| `E0207` | A view shows an element the model does not declare. |
| `E0208` | A FEEL expression does not parse. |
| `E0209`–`E0212` | A decision table is missing, malformed, or mismatched on inputs/outputs. |
| `E0213` | A knowledge-model invocation does not bind. |
| `E0215` | An emitted event is not declared in a provided contract. |

## Compatibility

- The model is one versioned document; its `version` is the compatibility
  contract consumers pin.
- FEEL texts are stored and syntax-checked, never evaluated by the surface.
- The artifact carries the graph (inputs, decisions, requirements, services) and
  the table provenance, so a consumer can rebuild the model without the sources.
