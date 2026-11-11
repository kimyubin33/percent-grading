# Changelog

## 0.3.0 — 2026-11-12

- Added bounded Crossref bibliographic discovery with DOI metadata and verifiable snapshots.
- Added sourced catalogs with explicit assumptions, dimensions, hypotheses, and overlap groups.
- Added deterministic selection from the task before seeing the answer, with count and context-byte limits.
- Suppressed overlap groups and exact duplicate content without adding scoring weight.
- Bound selected research, catalog snapshots, and selection diagnostics into the grading run identity.
- Preserved replay of 0.2 records without rewriting their identities or results.
- Added an eight-perspective apple catalog, research guide, and textbook chapter 15.
- Passed 80 local Rust tests, formatting, and Clippy, including a 10,000-entry capacity test.

The live Crossref connection was unavailable in the local execution environment. CI includes a non-blocking public connectivity smoke check; inspect that step separately from the offline test results. No paid AI evaluations were performed. Discovery returns candidates for source inspection, not validated theories.

## 0.2.0 — 2026-11-12

The project now has a Rust executable that compiles an actual RelSpec/AHAA contract, calls exactly OpenAI and Claude, validates categorical evidence, and produces a deterministic score profile.

- Pinned and integrated the upstream AHAA compiler without changing its source.
- Added strict policy, criterion, anchor, and perspective records with cross-reference checks.
- Fixed all possible numeric awards before evaluation. Model responses cannot add score fields.
- Separated current fulfillment from future hypothesis quality.
- Preserved disagreement as ranges and review status instead of averaging.
- Added official structured-output API adapters with bounded requests and credential preflight.
- Added content-addressed input records, cached provider responses, interrupted-run resume, and offline replay.
- Added four manually authored apple examples and textbook chapter 14.
- Verified 48 Rust tests, formatting, Clippy, and the 16 legacy Python tests locally.

No live API accuracy or educational benefit is claimed. The fixture results are authored examples, and two agreeing models can still be wrong. Automatic theory retrieval and human adjudication remain planned.

## 0.1.1

Initial public Python teaching prototype, bilingual introductions, research documentation, and English textbook.
