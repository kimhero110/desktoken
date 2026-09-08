# Change Control

## Purpose

Protect PRDCA discipline from uncontrolled scope, architecture, data, AI, and security drift.

## Change Levels

### Minor Change

Examples:

- Text correction
- Small UI label change
- Non-behavioral documentation improvement
- Configuration value with no acceptance impact

Approval: Project Manager or Product Owner.

PRDCA impact: Record the change; no return to P required.

### Moderate Change

Examples:

- Small feature adjustment within approved scope
- Non-breaking API behavior refinement
- Additional test case or metric
- Data field naming improvement without schema impact
- New local, reversible project skill with no external side effects
- Existing skill workflow update that does not expand security, data, or production authority

Approval: Product Owner, Tech Lead or Architect, and QA Lead.

PRDCA impact: Record the change and update affected plan/test artifacts.

### Major Change

Examples:

- Scope expansion or deletion affecting stage objective
- Architecture change
- Data model or ontology change
- AI/RAG behavior change affecting answers or citations
- Security, permission, privacy, or audit change
- Acceptance threshold change
- Schedule, cost, or staffing impact
- New external system integration
- Skill behavior that changes domain semantics, AI answers, data handling, permissions, production actions, destructive operations, or paid-service use

Approval: Return to P, update the plan package, and pass R again.

## Change Request Minimum Fields

- Change title
- Requester
- Date
- Current PRDCA stage
- Change level: minor, moderate, or major
- Reason
- Affected requirements
- Affected architecture/data/AI/security/QA areas
- Risk if accepted
- Risk if rejected
- Required approvals
- Final decision

## Enforcement

If the change level is uncertain, classify it as the higher level until reviewed.

No implementation task should start for a major change before revised P and R approval.
