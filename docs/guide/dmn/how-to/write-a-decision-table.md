---
title: Write a decision table
description: Put a decision's rules in an external markdown table, keep its columns matched to the decision's contract, and know what is never an output.
type: how-to
language: dmn
section: how-to
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/dmn/overview.md, docs/guide/dmn/tutorials/your-first-decision-model.md]
related: [docs/guide/dmn/reference/decision-models.md]
lastReviewed: 2026-11-21
---

# Write a decision table

Use this page when a decision's rules are best read as rows — thresholds,
ladders, and eligibility grids — and must stay matched to the decision's
declared inputs and outputs.

## Problem

Rules written in code are invisible to the people who own them, and rules
written in a document are never checked against the code that uses them. A
table gives the rules to the domain expert; the model has to keep the table
honest.

## Prerequisites

- A decision model with at least one decision.
- The table file in the model's directory (this page uses
  `examples/support/tables/refund-authority.md`).
- The [tutorial](../tutorials/your-first-decision-model.md) completed.

## Solution

**1. Declare the decision's output contract.**

```dmn,fragment
decision RefundAuthority {
    outputs {
        requires_approval: boolean;
        approver_role: string;
        maximum_amount: int;
    }
    logic table "./tables/refund-authority.md"
}
```

**2. Declare the information requirements — they are the table's inputs.** A
table-logic decision's information-required inputs must be exactly the table's
input columns, so the requirements block is part of the contract.

```dmn,fragment
requirements {
    information {
        RefundAuthority requires requested_cents;
    }
}
```

**3. Write the markdown in the dsntk shape.** Optional `>` header lines, then
exactly one table. The first header cell is the hit policy; the second row marks
each column `` `i` `` (input), `` `o` `` (output), or `` `ann` `` (annotation).

```markdown
> # Refund approval authority
> Who must approve a refund and how much each authority level may release.

| U | requested_cents | requires_approval | approver_role | maximum_amount |
|:-:|:---------------:|:-----------------:|:-------------:|:--------------:|
|   |       `i`       |        `o`        |      `o`      |      `o`       |
| 1 |     <=5000      |       false       |  "agent"      |      5000      |
| 2 | [5001..100000]  |       true        |  "finance"    |     100000     |
| 3 |     >100000     |       true        |   "board"     |    1000000     |
```

Cells are FEEL unary tests: quoted strings, ranges, comparisons, and `-` for the
wildcard. Column names are FEEL names and must match the decision's names
character for character.

**4. Check the model.**

```sh
pherolang check refunds.dmn
```

A missing table is `E0209`; a column mismatch is `E0211` (input) or `E0212`
(output). Annotation columns are supporting prose and are never treated as
outputs — the classic ingest defect this contract prevents.

## Example

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

The decision's three outputs are the table's three output columns, and its one
information-required input is the table's one input column.

## Related reference

- [Decision models](../reference/decision-models.md) — the decision and
  requirement syntax.
- [Decision services](../reference/decision-services.md) — exposing the result.
- [The .dmn language reference](../../../../DMN.md#external-decision-tables) —
  the full table contract, including hit policies and the annotation rule.
