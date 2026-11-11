# Roadmap

## 0.1.1 — Python teaching prototype

The initial implementation generated evaluation requests, validated manual JSON judgments, and computed fixed-rubric scores. It included bilingual introductions, an English textbook, and 16 local tests. It remains available for the original laboratory exercises.

## 0.2.0 — Rust contract and two-provider evaluation

- Use the actual pinned RelSpec/AHAA compiler and add a strict grading-specific semantic profile.
- Freeze criteria, assumptions, anchors, weights, and declared perspectives in a checked contract.
- Connect only OpenAI Responses and Anthropic Claude Messages.
- Require anchor IDs and answer evidence instead of model-authored numerical grades.
- Preserve two-provider disagreement for review.
- Keep current fulfillment separate from future exploration.
- Save records for replay and include offline manual fixtures.

Software tests and manually authored fixtures verify specified behavior. They do not establish live-provider reliability, grading accuracy, or educational benefit.

## Next — Controlled live validation

Run public, consented examples using explicit supported model versions. Record requests, validated responses, failures, latency, token usage, and observed cost. Test valid alternatives, unsupported conclusions, conditions, prompt injection attempts, refusal, and truncation.

Acceptance: publish a reproducible run manifest and reviewed failure cases. Distinguish live model judgments from authored fixtures and software tests.

## Next — Review and adjudication

Develop an explicit review workflow for disputed anchors. Preserve the two original judgments, reviewer rationale, and any new policy version. A reviewer may identify a broken rubric, a model error, or a genuinely unresolved case.

Acceptance: an amended outcome has a traceable reason and cannot silently replace its original evidence. Re-evaluate all affected answers after a rubric change.

## Next — Perspective retrieval and selection

Extend the declared perspective registry with sourced retrieval, relevance criteria, versioning, and overlap checks. Adding a perspective must not add weight to an unchanged criterion.

Acceptance: compare small and larger relevant perspective sets under controlled budgets, including duplicated and irrelevant lenses. Publish cases where more perspectives harm performance.

## Next — Domain study

Collect independently reviewed tasks and split evaluation by underlying task. Compare ordinary analytic rubrics, a single overall AI score, and the proposed profile. Measure criterion error, false rejection of valid alternatives, critical failures, coverage, useful feedback, latency, and cost.

Acceptance: publish the study design, limits, disagreements, and unfavorable results. Expand the number of theories only when measured gains justify the added cost.
