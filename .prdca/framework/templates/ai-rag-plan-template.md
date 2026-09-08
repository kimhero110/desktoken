# AI/RAG Plan Template

## Stage

[Stage number and name]

## AI Use Case

[Extraction, retrieval, answer generation, summarization, classification, etc.]

## Model Strategy

- Model/provider:
- Local or API:
- Fallback:
- Data privacy constraints:

## Retrieval Strategy

- Index type:
- Chunking strategy:
- Metadata filters:
- Ranking/reranking:
- Citation rules:

## Prompt and Tool Strategy

- System behavior:
- Required refusal conditions:
- Evidence requirements:
- Output format:

## Evaluation Set

| Case ID | Question/Input | Expected Evidence | Expected Behavior |
|---|---|---|---|
| E-001 | [Input] | [Evidence] | [Behavior] |

## Quality Metrics

| Metric | Target | Blocking |
|---|---|---|
| Answer correctness | [Target] | Yes/No |
| Citation accuracy | [Target] | Yes/No |
| Hallucination rate | [Target] | Yes/No |

## Risks

| Risk | Mitigation |
|---|---|
| [Risk] | [Mitigation] |

## Review Readiness

- [ ] AI use is necessary and bounded
- [ ] Evidence strategy defined
- [ ] Evaluation method defined before implementation
- [ ] Failure/refusal behavior defined
