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

## 0.3.0 — Bibliographic discovery and frozen perspective selection

- Search the fixed Crossref metadata endpoint for bounded bibliographic candidates.
- Validate versioned local catalogs with source references and explicit assumptions.
- Select perspectives from the checked task without inspecting the learner's answer.
- Limit selected context by count and byte budget; suppress repeated declared overlap groups.
- Preserve keyword matches, criterion mappings, exclusions, and uncovered criteria.
- Freeze the catalog and selection into a verifiable pack that becomes part of each grading run.
- Provide a small sourced apple catalog and textbook chapter 15.

Search results remain candidates until a curator inspects the underlying source. Lexical matching and overlap labels do not establish semantic relevance or scientific validity. Input-capacity tests are not a benchmark of theory coverage.

## 0.4.0 - Claim-level evidence and common reassessment

- Compile a separate evidence policy through the actual RelSpec/AHAA language and bind it to the grading task.
- Link required claims to exact imported source excerpts and explicit conditions.
- Require declared supportive and counterevidence search records without treating those records as proof of executed or exhaustive searches.
- Ask only OpenAI and Claude to review each link for support, contradiction, applicability, and source eligibility.
- Preserve insufficient, conflicted, contradicted, and future-hypothesis states rather than inventing trust probabilities.
- Collapse declared common origins and exact duplicate source text when counting independent support.
- Withhold affected final grades in evidence mode even when the answer judges agree on full credit.
- Save and replay source-review runs, compare evidence versions, and plan reassessment for every declared cohort member without overwriting old results.
- Include synthetic protocol fixtures, bilingual introductions, and textbook chapter 16.

This is a source-review protocol and deterministic enforcement layer. It does not authenticate imported web text, prove the independence of all sources, guarantee retrieval completeness, or establish scientific truth. The revision plan tracks pending work; it does not automatically execute reassessments.

## 0.5.0 - Benchmark experiments and application interfaces

- Validate versioned benchmark suites with complete criterion expectations, task-level development/holdout assignment, and within-task transformation checks.
- Include ten authored tasks and sixty answer cases, plus manual result fixtures with deliberate failures.
- Report accepted-anchor errors, withholding, wrong provider agreement, relation outcomes, and fresh repeated-trial stability against authored expectations.
- Plan and run bounded two-provider experiments with immutable trial identities, conservative attempt reservations, interrupted-run resume, and saved-result verification.
- Expose the common Rust engine through a versioned JSON service, a loopback HTTP adapter, and a JavaScript client with TypeScript declarations.
- Provide offline Rust/Node.js examples, English/Korean introductions, and textbook chapter 17.

The corpus is small and publicly authored. It exercises the experiment protocol, not independent grading validity. No paid provider evaluation is claimed. The local API adapter leaves user accounts, task authorization, production hosting, and application-level controls to its host.

## 0.6.0 - Historical failure regression and checked achievements

- Review primary research on scoring proxies, adversarial answers, biased reference labels, incomplete tests, and the limits of formal checking.
- Add 24 authored adversarial answers with paired transformations and deliberately failing manual results.
- Bind acceptance thresholds to an exact suite and check overall, split, and per-category results, coverage, wrong agreements, missing categories, and relation failures.
- Apply a 99.9 official score ceiling without altering raw anchors or silently migrating earlier reports.
- Compile a separate RelSpec proof challenge and independently verify complete truth-table certificates within a bounded Boolean domain.
- Return a separate, explained 10,000% celebration only for a checked certificate and fully settled maximum current anchors.
- Expose these operations through the existing application boundaries and add textbook chapter 18.

The gate detects specified failures in supplied results; it does not prevent every semantic model error. The proof checker is deliberately finite and does not decide arbitrary mathematical proofs, originality, or natural-language truth. Challenge approval, independent benchmark adjudication, real-provider evaluation, and classroom authorization remain host or research responsibilities.

## Next — Controlled live validation

Use the experiment runner with independently reviewed public or consented examples and explicit supported model versions. Record requests, validated responses, failures, latency, token usage, and observed cost. Test valid alternatives, unsupported conclusions, conditions, prompt injection attempts, refusal, and truncation. Distinguish a small protocol fixture from a sufficiently reviewed evaluation corpus.

Acceptance: publish a reproducible run manifest and reviewed failure cases. Distinguish live model judgments from authored fixtures and software tests.

## Next — Review and adjudication

Extend the evidence-version reassessment plan into an explicit review workflow for disputed anchors and completed adjudications. Preserve the two original judgments, reviewer rationale, and any new policy version. A reviewer may identify a broken rubric, a model error, or a genuinely unresolved case.

Acceptance: an amended outcome has a traceable reason and cannot silently replace its original evidence. Re-evaluate all affected answers after a rubric change.

## Next — Evaluate and improve research selection

Evaluate the implemented catalog selector, bibliographic discovery, and claim-level evidence review workflow. Measure source eligibility, applicability, false support, missed contradictions, and valid-alternative rejection against independent review. Adding a perspective must not add weight to an unchanged criterion.

Acceptance: compare small and larger relevant perspective sets under controlled budgets, including duplicated and irrelevant lenses. Publish cases where more perspectives harm performance.

## Next — Domain study

Collect independently reviewed tasks and split evaluation by underlying task. Compare ordinary analytic rubrics, a single overall AI score, and the proposed profile. Measure criterion error, false rejection of valid alternatives, critical failures, coverage, useful feedback, latency, and cost.

Acceptance: publish the study design, limits, disagreements, and unfavorable results. Expand the number of theories only when measured gains justify the added cost.
