# Project Skill Lifecycle

## Purpose

Convert repeated, stable project work into reusable agent skills while preventing duplicate, premature, or unsafe skills. A governed skill is written once and installed identically for every agent runtime the project uses.

Skills are governed capability assets. They complement code, tests, templates, and runbooks; they do not replace them.

## Discovery Points

Evaluate skill opportunities during:

- P, when repeated work is forecast and an existing skill may be reusable.
- R, when reviewers identify missing reusable controls or duplicated methods.
- C, when recurring defects or repeated manual checks appear.
- A, when lessons learned reveal a stable workflow worth preserving.
- Meta-review, when several cycles show the same cost or failure pattern.

## Candidate Threshold

A candidate requires at least two evidence signals:

- Same workflow used in at least three cycles.
- Same instructions or domain rules reconstructed at least three times.
- Same avoidable defect recurred at least twice.
- Same script, template, or checklist rewritten at least twice.
- Specialized knowledge is repeatedly rediscovered.
- Reuse has a measurable expected benefit in time, context, quality, or risk.

Reject or defer candidates that are one-off, generic, volatile, already covered, or cannot be validated.

Run the deterministic candidate gate before creation:

```text
prdca learning-check --input .prdca/framework/templates/learning-candidate-template.json
```

The gate requires two independently referenced signals that meet their occurrence thresholds, a measurable benefit, a completed overlap check, and a validation plan. A passing Skill candidate enters `trial`; it is not immediately `active`. When overlap exists, update the existing trial asset instead of creating a duplicate.

The primary AI may record candidate evidence, but the candidate decision does not certify that its own outputs are correct. References must point to completed runs, accepted artifacts, confirmed defects, or review evidence. Major semantic, security, production, or data-handling Skills require explicit human authority.

## Lifecycle

```text
candidate -> approved -> trial -> active -> needs-review -> active
     |           |          |         |             |
     +-> rejected+-> deferred+-> retired <----------+
```

1. Discover and record evidence.
2. Decide whether to create, update, defer, reject, merge, or retire.
3. Define triggers, exclusions, ownership, resources, and success metrics.
4. Create or update the skill in every path listed under `paths.skills`, using the `evolve-project-skills` skill.
5. Validate structure with `prdca skill-check` and run at least two realistic forward tests.
6. Keep new skills in trial until three successful real uses are recorded.
7. Promote, revise, merge, or retire during A or meta-review.

## Creation Rules

- Prefer updating an existing skill over creating overlapping skills.
- Use task-oriented, lowercase hyphenated names.
- Keep the first version small and operational.
- Add scripts only for repeated deterministic operations.
- Add references only for specialized knowledge required at execution time.
- Add assets only when the skill uses them in outputs.
- Never include credentials, secrets, transient reports, or unapproved sensitive data.
- Store project skills in every path listed under `paths.skills` in `prdca.json` and keep the copies byte-identical. The default targets are `.claude/skills/` and `.codex/skills/`.
- Reference framework assets as `.prdca/framework/...`; repository-relative paths do not resolve inside an adopting project.
- Record every skill in the registry.

## Risk and Approval

| Change | Default Level | Approval |
|---|---|---|
| Clarify an active skill without changing its trigger or behavior | Minor | Project Manager or skill owner |
| Create a local, reversible project skill with no external side effects | Moderate | Architect or Product Owner plus QA |
| Change a skill that controls domain semantics, AI answers, or data handling | Major | Return to P/R with Domain, AI/Data, and QA approval |
| Add production, security, paid-service, or destructive actions | Major | Sponsor, Security, Operations, and QA approval |

Creating or updating local, reversible project skills requires `authorization.local_skill_changes` in the adopting project's `prdca.json`, in addition to the candidate threshold and Moderate approval evidence. Major skills still require an explicit decision point.

## Validation Gate

A trial skill must provide:

- Valid `SKILL.md` frontmatter and naming.
- Current `agents/openai.yaml` metadata.
- A passing structural validation for every configured target:

```text
prdca skill-check --path .claude/skills
prdca skill-check --path .codex/skills
```

- At least two realistic forward-test results.
- No regression in output quality or safety.
- A measurable or clearly evidenced efficiency benefit.
- Independent validation for semantic, security, or production-impacting behavior.

## Effectiveness Metrics

Track at least one efficiency metric and one quality guardrail:

| Efficiency | Guardrail |
|---|---|
| Reduced completion time | Output acceptance rate does not fall |
| Fewer repeated steps | Defect rate does not increase |
| Lower context use | Required evidence remains complete |
| Fewer recurring defects | No new P0/P1 risk |

## Review and Retirement

Review active skills after a related process change, a recurring failure, or five relevant cycles. Mark a skill `needs-review` when its instructions may be stale. Retire or merge it when superseded, duplicated, unused for five relevant cycles, or repeatedly ineffective.

Registry: `.prdca/skills/registry.md`, created by `prdca init`.

Candidate template: `.prdca/framework/templates/skill-candidate-template.md`.
