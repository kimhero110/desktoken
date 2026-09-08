# PRDCA 2.0 Software Production System

## Purpose

Produce software through value-aligned planning, accountable execution, risk-routed review, proof-carrying delivery, evidence-first arbitration, and continuous capability learning.

The system is designed for one strong primary AI. Additional AIs are optional review instruments, not peers with automatic voting rights.

## Operating Model

```text
Human governance
    -> objective, constraints, risk authority
Value gate
    -> thesis alignment, user capability, evidence level, WIP, stop rules
Primary AI
    -> plan, implementation, continuity, delivery accountability
Deterministic controls
    -> tests, schemas, traceability, policy checks
Optional independent reviewers
    -> narrow findings from isolated review packets
Evidence arbiter
    -> block, human review, conditional pass, or pass
Learning loop
    -> tests, rules, skills, process revision
```

## Constitutional Rules

1. One primary AI is accountable for end-to-end delivery continuity.
2. Review intensity is selected by risk, not by a fixed number of models.
3. External AI is never mandatory when an equivalent independent control or human review is available.
4. Reviewers produce findings with evidence; they do not vote on truth.
5. Deterministic failures and substantiated P0/P1 findings override model agreement.
6. Material dissent is preserved and routed to the appropriate human authority.
7. Tests originate from approved requirements, not only from implementation structure.
8. Every delivery carries requirement, change, test, risk, rollback, and decision evidence.
9. Reviewer models retain roles only when measured results show unique value.
10. Repeated learning is automated first, then expressed as rules or skills when appropriate.
11. Local activity cannot substitute for user, workflow, or end-to-end value evidence.
12. Macro milestones and bounded work packages use separate identifiers.
13. Unresolved product decisions propagate a block to affected work.
14. Direction reviews and diminishing-return stop rules constrain continued investment.
15. Reusable capabilities are created only after repeated evidence, overlap checks, measurable benefit, and risk-appropriate approval.
16. System proposals, generated labels, or repeated outputs cannot certify their own correctness.

## Five Control Planes

### Value Plane

Every work package links to an approved project thesis, a macro milestone, a beneficiary, an observable problem, a user capability, and measurable success criteria. At least one metric must be evaluated at end-to-end, workflow, or observed-user-task level.

Use `.prdca/framework/governance/value-governance.md` and run:

```text
prdca value-check --input .prdca/framework/templates/value-assessment-template.json
```

### Governance Plane

Humans own objectives, budget, regulatory obligations, business value, and accepted residual risk. The primary AI may recommend but cannot silently expand these authorities.

### Execution Plane

The primary AI maintains context, evaluates alternatives, implements approved scope, records deviations, and produces the Delivery Evidence Pack.

### Verification Plane

Verification combines:

- Deterministic tests and policy checks.
- Fresh-context self-review for low-risk work.
- Isolated independent review for selected higher-risk work.
- Human approval for critical risk or value decisions.

### Learning Plane

The A stage decides whether repeated knowledge should become:

```text
automated test -> static rule -> reusable skill -> retained judgment -> human authority
```

Use `prdca learning-check --input .prdca/framework/templates/learning-candidate-template.json` before creating or updating a governed learning asset.

## Risk Routing

Assess six dimensions from 0 to 3:

- Impact.
- Irreversibility.
- Novelty.
- Semantic ambiguity.
- External exposure.
- Low testability.

Critical flags such as production release, security-boundary changes, destructive operations, credentials, regulated decisions, paid commitments, or irreversible migrations force L3.

| Tier | Default route |
|---|---|
| L0 | Primary AI plus automated checks |
| L1 | Primary AI plus fresh-context isolated review |
| L2 | One independent review slot plus evidence arbitration |
| L3 | Two independent review slots plus human risk acceptance |

An independent slot may be filled by a suitable external AI, a separately isolated reviewer, or a human. Missing slots must be explicit; they cannot be represented as completed reviews.

L1 fresh-context self-review is isolated but is not reported as an independent reviewer. External AI selection is allowed only when `external_review_allowed` is explicitly true in the risk assessment.

Run:

```text
prdca route --input .prdca/framework/templates/risk-assessment-template.json
```

## Delivery Evidence Pack

Every governed delivery records:

- Objective and accountable owner.
- Risk assessment and selected route.
- Requirements and priority.
- Requirement-to-change mapping.
- Requirement-to-test mapping and results.
- Assumptions and decisions.
- Review findings and unresolved dissent.
- Risks, mitigations, and rollback.
- Human approvals when required.
- Artifact paths with commit or digest evidence.
- Skill candidates and learning decisions.

Validate:

```text
prdca validate --input .prdca/framework/templates/delivery-evidence-pack-template.json --base-dir .
```

## Independent Review

Use `.prdca/framework/governance/independent-review-protocol.md`.

Reviewers receive a frozen artifact packet and a narrow task. They must not receive the author's hidden reasoning or another reviewer's conclusions before submitting their first finding set.

## Evidence Arbitration

Use `.prdca/framework/governance/evidence-arbitration.md`.

Arbitration is deterministic and does not count model votes. One substantiated P0/P1 finding can block delivery. Unsupported opinions remain advisory.

## Reviewer Governance

Each adopting project owns a `.prdca/reviewers.json` registry. Selection excludes unavailable reviewers and the primary model family, then ranks eligible reviewers by confirmed findings, structured-output reliability, false-block rate, and configured priority. The framework ships only a credential-free example.

Run:

```text
prdca select-reviewers --registry .prdca/reviewers.json --risk .prdca/framework/templates/risk-assessment-template.json --primary-family primary-family
```

Reviewer retention depends on unique confirmed findings, false positives, false blocks, structured-output success, latency, and cost. Agreement alone is not a performance metric.

## PRDCA Integration

### P

- Define thesis alignment, user capability, value evidence, WIP, assumptions, alternatives, acceptance tests, risk vector, and reviewer route.
- Freeze criteria before implementation.

### R

- Review the frozen plan through required lanes.
- Run independent reviews in isolation.
- Preserve dissent and arbitrate by evidence.

### D

- Implement only approved scope.
- Maintain requirement-to-change mapping and update the evidence pack.
- Return major discoveries to P.

### C

- Execute predeclared tests.
- Generate supplemental tests independently from requirements.
- Validate the evidence pack and arbitrate findings.

### A

- Decide accept, repair, replan, or stop.
- Compare value and quality outcomes with the original hypothesis.
- Update reviewer scorecards.
- Promote learning into tests, rules, or governed skills.
- Apply the stop rule when repeated cycles do not improve user-level evidence.
- Run a direction review at the configured interval.

## Adoption Rule

New projects start with `prdca init` and the files in `.prdca/framework/templates/`. Existing projects adopt the system incrementally: first risk routing and evidence packs, then independent review and reviewer scorecards. No historical rewrite is required.
