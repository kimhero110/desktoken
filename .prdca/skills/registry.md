# Project Skill Registry

This registry is the auditable record of every governed Skill decision in this project.
It is created by `prdca init` and maintained by the `evolve-project-skills` Skill.

Record one row per decision. Never delete a row; add a superseding row instead.

## Status Values

`candidate`, `trial`, `active`, `needs-review`, `retired`, `rejected`.

## Decisions

| Date | Skill | Action | Status | Candidate ID | Risk | Owner | Validator | Evidence |
|---|---|---|---|---|---|---|---|---|
| | | | | | | | | |

## Notes

- `Action` is exactly one of `create`, `update`, `defer`, `reject`, `merge`, `promote`, `retire`.
- `Candidate ID` links to the `prdca learning-check` input that authorized the action.
- `Evidence` links to forward-test results, `prdca skill-check` output, and the baseline comparison.
- The author may not be the sole `Validator` for semantic, security, or production-impacting Skills.
