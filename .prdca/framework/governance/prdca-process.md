# PRDCA Process

## Purpose

This process prevents uncontrolled implementation by requiring every stage to pass Plan, Review, Do, Check, and Act gates.

## Stage Flow

```text
P -> R -> D -> C -> A -> next stage or rework
```

The executable controls and evidence contracts are defined in `.prdca/framework/governance/software-production-system.md`.

## P: Plan

The plan team defines the stage package:

- Stage objective and business value
- Scope and non-goals
- PRD or requirement statement
- Technical approach
- Data and knowledge design
- AI/RAG/model approach when applicable
- Security approach
- QA and acceptance criteria
- Delivery plan
- Risks and rollback plan
- Existing skill reuse and skill-candidate assessment when repeated work is expected
- Risk vector and deterministic review route
- Delivery Evidence Pack initialized with requirement and acceptance-test mappings

Exit rule: P can exit only when the stage package is complete enough for formal review.

## R: Review

The review board checks the plan package through expert lanes:

- Product
- Domain
- Architecture
- Data governance
- Knowledge engineering
- AI/RAG when applicable
- Security
- QA
- Operations
- Delivery
- Capability reuse and skill lifecycle when applicable
- Independent review packets and context-isolation evidence required by the risk route
- Evidence arbitration without model voting

Exit rule: D may start only after required reviewers approve or explicitly grant non-blocking conditional approval.

## D: Do

The implementation team executes only the approved scope.

Rules:

- Major changes return to P.
- Minor changes are recorded.
- Implementation must map to approved requirements.
- Self-test evidence is required before C.
- Approved skill candidates may be created or updated under the project skill lifecycle.
- The Delivery Evidence Pack must maintain requirement-to-change mapping as implementation proceeds.

Exit rule: C may start only when implementation is complete, self-tested, and documented.

## C: Check

The quality team tests implementation against approved criteria.

Checks may include:

- Functional behavior
- Data quality
- Knowledge traceability
- AI/RAG accuracy and citation quality
- Security
- Performance
- Reliability
- Regression
- Documentation
- Skill structure, trigger behavior, and forward tests when a skill changed
- Delivery Evidence Pack validation
- Supplemental tests derived independently from approved requirements
- Reviewer findings and unresolved dissent arbitrated by evidence

Exit rule: A may start only when blocking quality gates pass.

## A: Act

The acceptance team decides whether to advance, repair, replan, or stop.

Outcomes:

- Accept and enter next stage
- Conditional accept with tracked fixes
- Return to D for implementation defects
- Return to P for plan or architecture defects
- Stop or rescope the stage
- Promote, revise, defer, merge, or retire skill candidates based on real-use evidence
- Update reviewer scorecards and retain only reviewers with measured contribution
- Decide which learning becomes an automated test, static rule, skill, retained judgment, or human authority

Defects may be closed only after a four-level closure: reproduce the observed failure, identify the earliest root cause and owning layer, audit sibling surfaces for the same mechanism, and prove recurrence prevention with an original regression, at least two meaning-preserving variants, full regression, and a production-shaped check. Run `prdca defect-check --input defect-closure.json` before closure.

## Rework Rules

- R failure returns to P.
- C failure usually returns to D.
- C failure caused by a wrong plan returns to P.
- A failure caused by unmet implementation returns to D.
- A failure caused by wrong objective, scope, architecture, data model, or AI approach returns to P.
