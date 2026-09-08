# Value Governance

## Purpose

Prevent locally successful delivery cycles from replacing product progress. A work package consumes implementation capacity only when it links to an approved project thesis, a macro milestone, a new or improved user capability, and measurable evidence.

## Naming Boundary

Use `M0`, `M1`, and similar identifiers for macro product milestones. Use `WP-001`, `DQ-001`, `UX-001`, `AI-001`, `OPS-001`, `SEC-001`, `DATA-001`, or `ARCH-001` for bounded work packages. Never create a new macro Stage merely to label a rule adjustment or local patch.

## Evidence Hierarchy

From weakest to strongest:

```text
local metric
-> unit test
-> component test
-> acceptance test
-> end-to-end test
-> workflow evidence
-> observed user task
```

Every work package requires at least one metric at end-to-end, workflow, or user-task level. Component quality remains necessary but cannot prove product value by itself.

## Capacity Controls

- Set and enforce a WIP limit.
- An unresolved decision that affects the work package blocks implementation.
- Run a direction review at the configured interval, normally every four cycles.
- When the configured number of consecutive cycles produces no user-level gain, stop, replan, or obtain explicit human authority to continue.

## Command

```text
prdca value-check --input .prdca/framework/templates/value-assessment-template.json
```

The combined gate may receive the same assessment with `prdca gate --value <path>`. Value validation runs before delivery evidence validation and arbitration.
