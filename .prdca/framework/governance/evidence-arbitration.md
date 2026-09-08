# Evidence Arbitration

## Principle

Decisions are based on evidence and authority, not reviewer count or model confidence.

## Decision Order

1. A failed deterministic test blocks delivery.
2. Any open, substantiated P0/P1 finding blocks delivery.
3. Critical-category findings require explicit risk handling.
4. Disputed domain, regulatory, business-value, or risk-acceptance questions go to human authority.
5. Substantiated P2 findings produce conditional pass unless an acceptance criterion makes them blocking.
6. Unsubstantiated opinions remain advisory.
7. Material unresolved dissent is preserved and cannot be summarized away.

## Evidence Standard

A substantiated finding needs:

- A concrete claim.
- Artifact, log, test, requirement, or reproducible behavior evidence.
- Severity and category.
- Current status.
- Affected requirement or risk when known.
- Independent evidence verification with `status: verified` and a verifier different from the finding author.

Model identity, confidence, eloquence, and agreement are not evidence.

Reviewers may propose `substantiated: true`, but the arbiter accepts that claim only after a separate evidence-verification record confirms it. An unverified claimed P0/P1 cannot directly block; it routes to human review until verified or rejected.

## Decisions

- `block`: deterministic failure or substantiated blocking defect.
- `human_review`: material authority or dissent decision is unresolved.
- `conditional_pass`: non-blocking confirmed corrections remain.
- `pass`: no substantiated blocker or material unresolved dissent remains.

Run:

```text
prdca arbitrate --input .prdca/framework/templates/arbitration-input-template.json
```

The arbitration report must retain all blocking, human-review, conditional, and advisory finding IDs.
