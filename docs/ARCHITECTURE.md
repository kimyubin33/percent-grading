# Architecture

## Intended pipeline

Define task and alternatives -> freeze rubric -> retrieve relevant perspectives -> group overlap -> evaluate evidence -> review disagreements -> aggregate one judgment per criterion -> report profile and uncertainty.

Only manual case input, request generation, validation, and aggregation are implemented. Automatic retrieval, model orchestration, and disagreement resolution remain planned.

## Perspective registry

Each record needs an ID, version, name, kind, source, relevance rationale, assumptions, associated criterion IDs, and overlap group. Distinguish established theories from proposed methods. The example registry is illustrative and is not loaded by the CLI.

Select perspectives from the task and a declared policy. Do not search for a lens that excuses a preferred response. Revise a rubric transparently when a legitimate alternative is discovered, then re-evaluate affected answers consistently.

For Q answers, k selected perspectives and r repeats, evaluation calls are approximately Q*k*r before verification and retrieval overhead. Measure error, coverage, latency, and cost before scaling to thousands of perspectives.

## Trust boundaries

Treat answers and retrieved text as untrusted data. Keep credentials and unrelated tools out of the judging context. Quotation checks and JSON parsing do not prove resistance to prompt injection.

Retain exact inputs, rubric and prompt versions, model identifiers, options, raw judgments and reviewed results. Automatic hashes and audit storage are future work.
