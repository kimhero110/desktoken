# RACI Matrix

## Role Definitions

- Sponsor: Business owner and final stage continuation authority.
- Product Owner: Scope, user value, product requirements.
- Project Manager: Schedule, gate control, decision records, action tracking.
- Solution Architect: Architecture, integration, technical coherence.
- Domain Expert: Engineering validation correctness and terminology.
- Data/Knowledge Lead: Data model, metadata, extraction schema, traceability.
- AI/RAG Lead: Retrieval, grounding, prompts, evaluation, model behavior.
- Security Lead: Access control, privacy, secrets, audit, compliance risk.
- QA Lead: Test strategy, defect policy, release recommendation.
- Implementation Lead: Delivery execution and technical task ownership.
- Operations Lead: Deployment, monitoring, backup, restore, runbooks.
- User Representative: Practical usability and acceptance feedback.

## RACI Legend

- R: Responsible for doing the work.
- A: Accountable for final decision or ownership.
- C: Consulted before decision.
- I: Informed after decision.

## Stage Responsibility

| Activity | Sponsor | Product | PM | Architect | Domain | Data/Knowledge | AI/RAG | Security | QA | Implementation | Operations | User Rep |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Project charter | A | R | R | C | C | C | C | C | C | I | I | C |
| Stage objective | A | R | R | C | C | C | C | C | C | I | I | C |
| PRD | C | A/R | C | C | C | C | C | C | C | I | I | C |
| Technical plan | I | C | C | A/R | C | C | C | C | C | C | C | I |
| Data/knowledge plan | I | C | C | C | C | A/R | C | C | C | C | I | I |
| AI/RAG plan | I | C | C | C | C | C | A/R | C | C | C | I | I |
| Security plan | I | C | C | C | I | C | C | A/R | C | C | C | I |
| QA plan | I | C | C | C | C | C | C | C | A/R | C | I | C |
| R approval | C | A | R | A | A | A | A | A | A | C | C | C |
| Implementation | I | C | C | C | C | C | C | C | C | A/R | C | I |
| Quality check | I | C | C | C | C | C | C | C | A/R | C | C | C |
| Stage acceptance | A | A | R | C | C | C | C | C | A | C | C | C |
| Change control | A for major | A/R | R | C | C | C | C | C | C | C | C | I |
| Skill lifecycle | I | C | R | A | C | C | C | C | C | R | I | C |

## Separation of Duties

- The implementation lead cannot be the sole approver of implementation quality.
- QA has independent authority to block A.
- Security has independent authority to block high-risk release.
- Domain expert has authority to block incorrect engineering conclusions.
- Sponsor decides whether accepted residual risk is tolerable.
- A skill author cannot be the sole validator of a semantic, security, or production-impacting skill.

## AI Execution Roles

- Primary AI: Responsible for planning continuity, implementation, evidence-pack maintenance, and delivery synthesis. It is not the sole approver of its own L2/L3 work.
- Independent Reviewer: Responsible only for the assigned narrow review and evidence-backed findings. It has no automatic release vote.
- Challenger: Responsible for counterexamples, failure modes, and missing caveats on selected high-risk work.
- Evidence Arbiter: Applies deterministic decision rules and preserves dissent; it does not generate new factual claims.

External AI reviewers are optional. Human role accountability in the RACI table remains authoritative for business value, domain meaning, security, operations, and accepted residual risk.
