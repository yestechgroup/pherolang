# Pull request

## Summary

<!-- What changes, and why. Link the issue when one exists. -->

## Documentation checklist

Every row is blocking. Rules are defined in
[docs/documentation/EDITORIAL_POLICY.md](docs/documentation/EDITORIAL_POLICY.md);
delete a row only when the change genuinely cannot affect it, and say why in
the summary. `[CI]` rows are also enforced by the documentation gate.

- [ ] **E14 — Feature documentation.** A change to syntax, semantics,
      validation, or an artifact updates the affected Guide pages, or the
      waiver and its follow-up issue are named here.
- [ ] **E15 — Semantics review.** The page that describes changed behavior is
      updated, and its `lastReviewed` is current.
- [ ] **E1/E2 — Page purpose and authority.** Any new page names its audience,
      question, and outcome; it does not duplicate an existing authoritative
      explanation (link instead).
- [ ] **E4 — Status honesty.** `status` and `docStatus` describe what exists,
      not what is planned.
- [ ] **E9/E10 — Standards provenance.** Standards-derived claims are labeled
      (industry standard / Phero implementation / extension / divergence /
      proposed), and no conformance is implied by resemblance.
- [ ] **E17/E18 — No silent drift.** Removed or renamed behavior has its pages
      updated, deprecated, or deleted.
- [ ] **CM10–CM11a — Examples.** `mox` fences compile; syntax illustrations are
      `mox,fragment`; invalid examples are `mox,invalid=<CODE>`.
- [ ] **CM13 — No development history.** Guide pages contain no issue numbers
      or decision narration; rationale is linked from an ADR.

## Verification

- [ ] `scripts/ci-local.sh` was run and every selected step reported `*-OK`
      (or the steps that do not apply are listed).
- [ ] Golden fixtures were regenerated with `PHERO_UPDATE_FIXTURES=1` and the
      diff was read (when a fixture changed).
- [ ] For site changes: `node site/scripts/gate.mjs` passed, or the failing
      check is named with its report path.
