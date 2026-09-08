# Independent Review Protocol

## Objective

Reduce inherited cognitive bias without replacing primary-AI accountability or creating model voting.

## Review Packet

Freeze and provide only:

- Approved requirements and acceptance criteria.
- Relevant design or changed artifacts.
- Constraints and risk assessment.
- Required evidence and known exclusions.
- A single narrow review assignment.

Do not provide hidden chain-of-thought, persuasive author commentary, prior reviewer conclusions, or an intended verdict.

## Independence Requirements

A review counts as independent only when:

- The reviewer did not author the submitted artifact.
- Its first pass is context-isolated from the author's reasoning and other findings.
- It reports evidence locations and reproducibility.
- It distinguishes observed facts, inferences, and recommendations.
- Its output follows `.prdca/framework/templates/independent-review-template.json`.

Different providers improve diversity but do not by themselves prove independence. A fresh isolated context may satisfy L1. L2/L3 should prefer a different model family or a human when available.

L1 fresh-context self-review must be recorded with `fresh_context: true` and must not claim `independent: true` when the same primary AI performs it.

## Narrow Review Assignments

- Requirement omission.
- Architecture constraint violation.
- Code and requirement mismatch.
- Independent test generation.
- Data lineage or semantic evidence check.
- Security and privacy challenge.
- Failure-mode and counterexample generation.

Do not ask a weaker reviewer to redesign the entire product or make the final release decision.

## Two-Pass Process

1. Independent pass: each reviewer submits findings without seeing other reviews.
2. Evidence pass: the primary AI validates evidence and reproduction.
3. Rebuttal pass: reviewers may respond to evidence, not to model status or confidence.
4. Arbitration: deterministic policy produces the gate decision.

## Data Disclosure

Before external review:

- Confirm the selected risk route allows external processing.
- Send only the frozen minimum packet.
- Remove credentials and unapproved sensitive data.
- Record provider, model, payload scope, and timestamp.
- Use human or local review when data cannot leave the approved boundary.

## Invalid Review Patterns

- Majority voting.
- Reviewer sees the intended answer before its first pass.
- Agreement treated as evidence.
- Parsing failure treated as approval.
- Missing independent slots silently ignored.
- Reviewer confidence used instead of reproducible evidence.
