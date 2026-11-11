# Testing the grader with authored expectations

Version 0.5 adds a benchmark corpus, an offline evaluator, a bounded live experiment runner, and error reports. These tools test whether a grader follows the intended criterion-level behavior. They do not turn authored expectations into unquestionable ground truth or establish a universal grading accuracy.

## Start with the bundled corpus

The [benchmark directory](../benchmarks/README.md) contains ten English tasks and six answers per task: sixty cases in total. Seven tasks are assigned to development and three whole tasks to holdout. The tasks use elementary mathematics, logic, code interpretation, and an explicitly declared color label whose expected answers can be inspected directly.

Each case has a category, the learner answer, and allowed anchor IDs for every criterion. Some cases test different wording, useful fragments, unsupported conclusions, conditional alternatives, conflicting statements, authority claims, or embedded instructions. The corpus is an authored engineering fixture, not an independently adjudicated educational dataset.

```text
cargo run --locked -- benchmark-validate benchmarks/suite.json
cargo run --locked -- benchmark benchmarks/suite.json benchmarks/manual-results.json --out local-runs/benchmark.manual-report.json
```

The manual result file deliberately includes wrong agreements and withheld judgments so the report exercises failure paths. Its OpenAI/Claude fields identify protocol roles; the records were not produced by live model calls. **Metrics from this file describe the authored fixture only.**

Outputs use create-new behavior. Choose a new path or inspect the existing report instead of overwriting it.

## What belongs in the suite

A suite has a format identifier, ID, version, and tasks. Every task embeds its AHAA grading policy, its development/holdout assignment, answer cases, and optional pairwise relations. A task may also carry a checked evidence pack bound to that policy.

The validator compiles every policy through the real RelSpec/AHAA compiler. It checks task and case identities, limits, expected criterion coverage, anchor membership, relations, and any evidence binding. A repeated compiled contract cannot appear in both splits. This structural check cannot detect every semantically equivalent task disguised by different wording.

An expectation lists permitted anchor IDs for one criterion. The reserved `unresolved` value can express a deliberately expected withholding. Use a set only when more than one interpretation is justified; adding every anchor would make the check meaningless.

Authored expectations never enter the provider prompt. Live requests contain the fixed grading task, answer, and optional evidence pack. The harness compares responses with expectations only after the providers return.

## Relations test changes that should matter

Relations compare the same criterion between two cases in one task. They are not comparisons between two students' overall worth or ability.

| Relation | Meaning: right case compared with left case |
|---|---|
| `equal` | The accepted criterion points should remain equal |
| `not_lower` | The right case should not receive fewer accepted points |
| `not_higher` | The right case should not receive more accepted points |

For example, an equivalent rewording should preserve the relevant criterion, while adding an irrelevant authority claim should not increase it. These are authored expectations for a specific transformation. A corrected contradiction or genuinely added reasoning may appropriately change another criterion.

If either side has no settled award, the relation is indeterminate. The report does not count an unresolved comparison as a successful invariance check.

## Read error and coverage together

The report includes overall, split, and category summaries; trial-level grading reports; expectation checks; relation outcomes; and repeated-trial stability. Counts and denominators remain visible.

Pay attention to these different questions:

- How many criteria received an accepted award, and how many remained withheld?
- Among accepted awards, how many agreed with the authored permitted anchors?
- Where did both models choose the same unacceptable anchor?
- Which expected valid answers were rejected or left unresolved?
- Did a rewording or irrelevant addition change the accepted criterion points?
- Were repeated fresh judgments stable under the same task, answer, models, and settings?

A system that withholds every answer must not look perfect merely because it makes no incorrect final award. Conversely, an authored expectation of `unresolved` is a test of deliberate abstention, not an ordinary correct answer. The detailed checks preserve that distinction.

Rates against authored references are not calibrated truth probabilities. The report does not infer grading quality from provider agreement alone, and no-repeat experiments have no measured repeated-trial stability rate.

## Review a live experiment plan first

Choose model IDs that your provider accounts support. `benchmark-plan` validates the experiment and writes a plan without provider calls or API credentials:

```text
cargo run --locked -- benchmark-plan benchmarks/suite.json --split development --repeats 1 --max-api-calls 84 --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL --out local-runs/benchmark.plan.json
```

The development split has 42 cases. At one repeat and one attempt per provider, the initial upper bound is 84 provider attempts. The entire corpus has 60 cases and therefore requires an initial bound of 120 at those settings. These are request-attempt counts, not estimates of money or total tokens.

With `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` set, execute the corresponding experiment:

```text
cargo run --locked -- benchmark-run benchmarks/suite.json --split development --repeats 1 --max-api-calls 84 --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL --runs-dir local-runs/benchmarks
```

This command calls only OpenAI and Claude and can incur charges. The default is one request attempt per provider. Raising `--max-attempts` also raises the conservative required initial budget. A plan is rejected before calls if the budget cannot cover the requested cases, repeats, and retry allowance.

## Repeats, resume, and budgets

`--repeats` accepts one to five. A repeat gets a distinct experiment-trial identity while keeping the task/answer prompt unchanged. This deliberately requests a fresh judgment rather than replaying the first response. Re-running the same experiment resumes its existing trials instead of generating another repeat. Changing the suite, models, limits, split, repeat count, or budget creates a different experiment identity.

An experiment saves its manifest, conservative attempt reservations, individual grading runs, and, on completion, results and the derived report. Each provider request reserves its configured maximum number of attempts before transport. The reservation remains consumed even if the call fails. This may overcount attempts actually made, but prevents a resumed experiment from silently exceeding its declared request allowance.

Successful cached provider responses consume no new reservation. A failed experiment can continue only with its remaining allowance. Budget changes produce a new experiment; the program does not pool separate experiments into an account-wide spending limit.

An exact initial budget has no headroom after a failed reservation. Choose a finite allowance for possible retries before starting if needed. Completed results and reports are written only after every selected case and repeat finishes. A failure leaves an incomplete experiment with its manifest, reservations, and trial records; the harness does not silently drop failed cases to improve its reported accuracy.

Completion produces:

```text
RUNS/PLAN_ID/manifest.json
RUNS/PLAN_ID/reservations/000001.json
RUNS/PLAN_ID/trials/RUN_ID/input.json
RUNS/PLAN_ID/trials/RUN_ID/openai.response.json
RUNS/PLAN_ID/trials/RUN_ID/claude.response.json
RUNS/PLAN_ID/results.json
RUNS/PLAN_ID/report.json
```

The complete saved result is checked against the manifest, individual archived responses, and recomputed benchmark report before being reused. The raw records remain available for inspecting errors. Imported `api_run` labels are not provider signatures; only the verified local archives establish consistency with this runner's recorded workflow.

## Avoid tuning on the final check

Use development tasks to inspect failures and improve rubrics or prompts. Keep whole holdout tasks out of those changes. A rewording of the same underlying question is not an independent held-out task.

The bundled holdout cases are public and authored within the same small project. They demonstrate the split protocol; they are not a secret or independent benchmark. Stronger claims require a larger independently reviewed set, explicit inclusion criteria, and a record of revisions. Publish unresolved cases and unfavorable results alongside improvements.

The original evidence workflow remains optional. A benchmark without an evidence pack measures the grading contract and model judgments without asserting that a source audit took place. A benchmark with a pack applies the same deterministic evidence gate as ordinary evidence-mode grading.

## Next experimental step

Inspect the sixty authored cases and their criterion expectations before making paid calls. Correct task ambiguity first. Run the development plan with explicit models and bounded attempts, inspect every failure class, and preserve the resulting records. Use a separate held-out run to evaluate the final revision, without selecting only favorable model responses.
