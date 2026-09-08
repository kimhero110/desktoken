# QA Report Template

## Stage

[Stage number and name]

## QA Decision

Decision: [Pass to A / Return to D / Return to P]

May A start: [Yes/No]

## Test Scope

### Included

- [Included area]

### Excluded

- [Excluded area and reason]

## Test Evidence

| Artifact | Path/Reference | Notes |
|---|---|---|
| Test plan | [Path] | [Notes] |
| Test data | [Path] | [Notes] |
| Test results | [Path] | [Notes] |

## Lane Results

| Lane | Result | Metrics | Notes |
|---|---|---|---|
| Functional | Pass/Fail/NA | [Metrics] | [Notes] |
| Data quality | Pass/Fail/NA | [Metrics] | [Notes] |
| Knowledge quality | Pass/Fail/NA | [Metrics] | [Notes] |
| AI/RAG quality | Pass/Fail/NA | [Metrics] | [Notes] |
| Security | Pass/Fail/NA | [Metrics] | [Notes] |
| Performance | Pass/Fail/NA | [Metrics] | [Notes] |
| Reliability | Pass/Fail/NA | [Metrics] | [Notes] |
| Regression | Pass/Fail/NA | [Metrics] | [Notes] |
| Documentation | Pass/Fail/NA | [Metrics] | [Notes] |
| Evidence pack | Pass/Fail/NA | Requirement/change/test coverage | [Notes] |
| Independent review | Pass/Fail/NA | Required/actual isolated reviews | [Notes] |
| Arbitration | Pass/Fail/NA | Block/human/conditional/pass | [Notes] |

## Defects

| ID | Severity | Description | Owner | Status | Blocks A |
|---|---|---|---|---|---|
| DEF-001 | P0/P1/P2/P3 | [Description] | [Owner] | Open/Closed | Yes/No |

## Thresholds

| Metric | Target | Actual | Pass |
|---|---|---|---|
| [Metric] | [Target] | [Actual] | Yes/No |

## Residual Risks

| Risk | Severity | Accepted By | Notes |
|---|---|---|---|
| [Risk] | [Severity] | [Role] | [Notes] |

## Independent Test Generation

- Requirements used as input:
- Implementation details intentionally excluded:
- Supplemental tests added:
- New defects found:

## Reviewer Quality

| Reviewer | Confirmed Findings | Unique Findings | False Positives | False Blocks | Structured Output |
|---|---:|---:|---:|---:|---:|
| [Reviewer] | 0 | 0 | 0 | 0 | 100% |

## Final Statement

[State whether A may start and why.]
