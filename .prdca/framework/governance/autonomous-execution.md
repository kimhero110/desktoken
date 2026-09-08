# Autonomous Execution Mode

## Status

Governed by the `authorization` block in the adopting project's `prdca.json`. The framework
grants nothing by itself; each project's owner sets the flags. Treat any flag that is absent or
`false` as not granted.

| Flag | Grants |
|---|---|
| `standing_execution` | Advance stages and record gate decisions when the gate evidence exists |
| `routine_document_updates` | Create or update routine governance, review, QA, acceptance, roadmap, and template documents |
| `non_destructive_commands` | Run validation and inspection commands |
| `local_skill_changes` | Create or update local, reversible project skills once the governed candidate threshold and approval evidence are satisfied |
| `destructive_filesystem_or_git` | Destructive filesystem or git operations |
| `external_deploy_or_publish` | External deployment, network dependency installation, or publication |

The sections below describe what each flag permits. They do not themselves grant authority.

## Objective

Continue remaining project work without waiting for confirmation unless a true decision point or external blocker appears.

## Default Decision Policy

When several paths are available, choose the path that is:

1. Lowest operational risk.
2. Locally runnable and testable.
3. Reversible through file-level changes.
4. Consistent with existing project architecture.
5. Most useful for engineering knowledge quality before platform complexity.

## Automatic Choices

With `standing_execution`, `routine_document_updates`, and `non_destructive_commands` granted,
the agent may automatically:

- Create PRDCA documents for each stage.
- Implement local deterministic pipelines.
- Add tests and run test suites.
- Regenerate derived reports and dashboards.
- Add decision records under `docs/decisions`.
- Use local file-based outputs before introducing services or databases.
- Propose, create, or update local project skills when the governed candidate threshold is met and `local_skill_changes` is granted.
- Validate skill structure with `prdca skill-check`, run bounded forward tests, and maintain the project skill registry.
- Run deterministic PRDCA risk routing, evidence-pack validation, and evidence arbitration.
- Select optional reviewers from the reviewer registry when the risk route requires independent review.

## Decision Points Requiring Stop or Defer

The agent must not silently perform the following, regardless of the flags above, unless the
corresponding flag is explicitly granted:

- Destructive file or git operations.
- Real production deployment.
- Paid/cloud infrastructure setup.
- Secret rotation or API key changes.
- External account purchases or quota changes.
- Broad architecture shifts such as mandatory database migration.
- Skills that add production actions, destructive behavior, paid services, new secrets, or broad security/data authority.
- L3 human risk acceptance or unresolved domain, regulatory, business-value, or safety authority decisions.

When these appear, record the decision and choose a local/deferred fallback where possible.

## Skill Evolution Policy

Use `.prdca/framework/governance/skill-lifecycle.md` and the `evolve-project-skills` project skill.

- Prefer updating an existing skill over creating overlap.
- Require at least two evidence signals before creation.
- Keep a new skill in trial until three real uses succeed.
- Do not let the skill author be the sole validator for semantic, security, or production-impacting behavior.
- Treat local reversible skill creation as a Moderate change; route higher-risk skills through P and R.

## Primary AI and Reviewers

- Keep one primary AI accountable for end-to-end delivery continuity.
- Treat additional AIs as optional, narrow reviewers rather than voting peers.
- Use a fresh isolated context for L1 and independent frozen review packets for L2/L3.
- Do not send credentials or unapproved sensitive data to external reviewers.
- If no eligible reviewer can fill a required slot, record the missing slot and use human or fresh-context fallback; never fabricate independence.
- Parsing failure, provider failure, or missing structured output cannot be interpreted as approval.

## Current Strategic Path

1. Finish ontology/context governance.
2. Add dashboard filtering and evidence drill-down.
3. Add risk and anomaly rules.
4. Add export package support.
5. Defer vector database and multi-user platform hardening until data quality is stronger.
