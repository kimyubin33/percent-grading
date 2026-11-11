# Changelog

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
