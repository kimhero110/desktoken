# Project Working Agreement

PRDCA is the base methodology for this project. Apply it to any non-trivial change,
in whichever agent runtime you are running.

## Sequence

```text
P: Plan -> R: Review -> D: Do -> C: Check -> A: Act
```

Advance a stage only when that stage's gate evidence exists. Convenience is not evidence.

## Skills

These Skills are installed in this project and are the entry points for the process.
They load from `.claude/skills/` in Claude and `.codex/skills/` in Codex; the content is identical.

| Skill | Use it for |
|---|---|
| `prdca-governance` | Stage control, gate decisions, change classification, routing |
| `review-board` | The R stage: reviewing a plan package before implementation |
| `qa-gate` | The C stage: quality lanes, defect triage, release blocking |
| `evolve-project-skills` | Turning repeated workflows into governed Skills |

Start with `prdca-governance` when you do not know which stage the work is in.

## Non-Negotiable Rules

- Gate decisions come from `prdca` commands, not from narrative judgement.
  Exit code `0` is pass, `2` is block, `3` is human review.
- Model agreement is not evidence. Reviewer AIs produce findings and never vote.
- One primary AI or team stays accountable for the delivery.
- A major change in scope, architecture, data, AI behavior, security, or acceptance
  criteria returns to P.
- The team that submits work cannot be the only team approving it.

## Authority

`prdca.json` holds this project's accountability, policy, and `authorization` block.
Read `authorization` before acting without confirmation, and treat every flag that is
absent or `false` as not granted.

## Contract

The normative contract is `.prdca/framework/governance/software-production-system.md`.
Templates are in `.prdca/framework/templates/`.
