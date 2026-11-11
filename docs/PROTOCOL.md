# Input contract

**Historical teaching protocol:** this page describes the retained Python prototype. Its raw outputs are not official grades under the current required examiner-review policy. Use Rust 0.7 and [HUMAN_REVIEW.md](HUMAN_REVIEW.md) for consequential grade release.

## Case

Required nonempty strings: case_id, rubric_version, question, answer, reference. policy.coverage_min is a number in [0,1]. criteria is a nonempty array.

Each criterion has a unique id, dimension, description, positive numeric weight, and boolean critical. A critical criterion also has critical_threshold in [0,1]. Define observable scoring anchors in description. Extra metadata is outside the supported contract.

## Evaluation

case_id and rubric_version must match the case. judge_id is a nonempty evaluator/run identifier. judgments contains exactly one row per criterion.

Each row has criterion_id, score, evidence, and reason. score is in [0,1] or null. evidence is an array of exact, nonempty answer substrings. A positive score requires at least one quote. Zero or null may have an empty array when the reason explains an omission or abstention.

A valid quote is not automatically relevant evidence. The validator checks existence, not meaning. judge_id is user-supplied metadata rather than authenticated provenance.

## CLI

Run prompt with a case file. Give its output to a chosen AI. Save only the returned JSON object as UTF-8, with or without a UTF-8 BOM. Run aggregate with the case and judgment paths.

Invalid inputs exit with code 2. Valid aggregation exits with code 0, including blocked and review statuses. A workflow state is not a parsing error.

Automatic AI calls are not included. Preserve raw model outputs for review and do not put private student data in public examples.
