# Stage Gates

## Gate Summary

| Gate | Required Decision | May Advance To |
|---|---|---|
| P Gate | Plan package complete | R |
| R Gate | Expert review approved | D |
| D Gate | Implementation complete and self-tested | C |
| C Gate | Quality gates passed | A |
| A Gate | Accepted, replanned, or stopped | Next stage or rework |

## P Gate Checklist

- Stage objective defined
- Business value defined
- Scope and non-goals defined
- Requirements documented
- Technical approach documented
- Data and knowledge approach documented
- AI/RAG approach documented when applicable
- Security approach documented
- QA and acceptance criteria documented
- Delivery plan documented
- Risks and rollback documented
- Existing skills reviewed for reuse; repeated workflows recorded as candidates when applicable
- Risk assessment completed and review route recorded
- Delivery Evidence Pack initialized
- Must requirements mapped to predeclared acceptance tests

Decision: pass to R or remain in P.

## R Gate Checklist

- Product review complete
- Domain review complete
- Architecture review complete
- Data/knowledge review complete
- AI/RAG review complete when applicable
- Security review complete
- QA review complete
- Operations review complete when applicable
- Delivery review complete
- Skill overlap, trigger boundary, risk, and validation plan reviewed when applicable
- Required independent reviews completed from frozen, isolated packets
- Findings contain evidence and material dissent is preserved
- Evidence arbitration permits D
- Blocking objections closed

Decision: pass to D, conditional pass to D, or return to P.

## D Gate Checklist

- Approved scope implemented
- Requirement-to-change mapping available
- Self-tests executed
- Known issues documented
- Run or deployment instructions available
- Approved project skill changes implemented and registered when applicable
- Requirement-to-change mapping is complete in the Delivery Evidence Pack
- Deviations, assumptions, and rollback triggers are current
- Major deviations absent or approved through change control

Decision: pass to C or remain in D.

## C Gate Checklist

- Functional tests pass
- Data quality tests pass
- Knowledge traceability tests pass
- AI/RAG tests pass when applicable
- Security tests pass
- Performance tests meet threshold
- Changed skills pass structural validation and realistic forward tests
- Delivery Evidence Pack validation passes
- Supplemental tests were derived independently from approved requirements
- Required independent review slots are satisfied or explicitly escalated
- Evidence arbitration permits A
- P0/P1 defects closed
- Defect closures pass `prdca defect-check`; point fixes without root cause, sibling-surface audit, and recurrence-prevention evidence remain open
- QA report recommends A

Decision: pass to A, return to D, or return to P.

## A Gate Checklist

- Stage outcome compared to objective
- User/business feedback reviewed
- Quality report reviewed
- Security and operations risks reviewed
- Residual risks accepted or rejected
- Lessons learned recorded
- Skill candidates promoted, revised, deferred, merged, rejected, or retired
- Reviewer contribution metrics updated
- Value hypothesis and quality guardrails compared with actual outcomes
- Learning disposition recorded as test, rule, skill, retained judgment, or human authority
- Next decision recorded

Decision: enter next stage, conditional fix, return to D, return to P, or stop.
