# Architecture

## Application and experiment interfaces

Version 0.5 adds two callers of the same Rust grading core. The `benchmark` and `benchmark_runner` modules validate authored suites, execute bounded independent trials, and report errors against explicit expectations. The `service` module accepts a versioned request envelope independently of HTTP; `server` provides a bounded loopback adapter. The CLI and JavaScript client call these interfaces without introducing separate score arithmetic.

Benchmark expectations remain outside provider prompts. An experiment trial adds a recorded identity to request the same answer afresh, while replay/resume retains that trial's first response. Attempt reservations are persisted before transport and remain consumed after failure. These controls apply within the declared experiment, not across all experiments or the provider account.

The service context supplies storage and the live-call permission. Clients supply complete checked objects and answer data, not filesystem paths or provider credentials. The HTTP boundary requires its own bearer token, rejects browser Origin requests, and binds only to loopback. Browser applications use their own backend for authorization and presentation. `null` final percentages and future/current separation must survive every presentation layer.

See the [benchmark guide](BENCHMARK_GUIDE.md) for dataset/metric semantics and the [API guide](API_GUIDE.md) for the operation envelope and integration limits.

## Two layers of constraints

The Rust implementation uses the real compiler from [RelSpec, now named AHAA](https://github.com/kimyubin33/relspec-lang). The upstream source is pinned at `de1e956555dcfafc5b62324ea6ed3f62655c6ba3`. Its parser and checker produce checked scene records. Percent Grading then applies an additional grading-specific semantic contract.

AHAA checks the language structure. Percent Grading checks whether that structure defines a usable rubric: recognized record kinds, required fields, valid references, fixed criterion dimensions, positive weights, permitted anchors, and supported provider identities. Passing the language checker alone does not establish that a rubric is complete, fair, or scientifically correct.

The native AHAA keywords remain Korean because they are part of the user's language. Example identifiers, descriptions, output, and technical documentation are English. The integration does not silently replace AHAA with a similar-looking custom parser.

## Authority and data flow

```mermaid
flowchart TD
    A["AHAA contract"] --> B["Compiler and grading validation"]
    B --> C["Fixed rubric and request identity"]
    C --> D["OpenAI judgment"]
    C --> E["Claude judgment"]
    D --> F["Evidence and anchor validation"]
    E --> F
    F --> G["Agreement or review"]
    G --> H["Deterministic report and audit record"]
```

The rubric author defines what is evaluated and how much each accepted anchor earns. The two models classify evidence using those anchors. Rust owns the point mapping, weights, bounds, and reporting rules. Models cannot introduce a new criterion, invent an anchor, change its points, or submit their own numerical grade.

The supported live providers are exactly OpenAI and Anthropic Claude. The program calls their official APIs directly. A ChatGPT web subscription is not an API credential. Provider model IDs are explicit configuration because API availability varies by account and over time.

## Agreement does not mean forced agreement

For each criterion, both judges must identify the same supported anchor before it becomes an accepted criterion result. A different anchor or unresolved judgment remains visible as disagreement. The program preserves a bounded result for inspection and withholds the affected final decision. It does not average two conflicting interpretations into an apparently settled grade.

There are three different guarantees:

| Layer | Guarantee | Limit |
|---|---|---|
| Input contract | Only supported structures and values reach aggregation | A well-formed statement can still be false |
| Numerical scoring | The same validated judgments and rubric produce the same arithmetic | An AI may classify an answer differently on a fresh call |
| Recorded replay | A valid saved run can be reused without asking the models to judge again | Replay preserves an earlier judgment; it does not prove that judgment correct |

Model agreement is evidence about agreement, not an independent probability of truth. Both systems may share a misconception. Exact quotation checks establish that a substring exists in the answer; they do not establish that it entails the chosen anchor.

## Present fulfillment and future exploration

The current-task profile distinguishes intent, principle, reasoning, and conditions. Future-oriented criteria describe explicit alternatives, hypotheses, proposed experiments, or possible developments. Future exploration is reported separately from current-task fulfillment so that a speculative possibility cannot repair a false claim about the present specimen.

Every future claim should name what would need to change and what evidence could test it. A scenario can earn credit for being clearly specified while remaining unverified. A rubric percentage is not the probability that the scenario will occur.

## Perspectives do not create extra votes

A perspective is a declared way to inspect a criterion. Relevant lenses can include perception, surface reflectance, causal reasoning, or experimental design. Their sources and assumptions belong in the contract and audit trail. The same answer is still evaluated against the fixed criteria.

Adding ten descriptions of the same physical mechanism does not create ten new shares of the grade. Version 0.3 selects bounded context from a sourced catalog using answer-independent keyword matching. It suppresses declared overlap groups and exact duplicate content, reports omitted records and coverage gaps, and freezes the selection into the run identity. The selector supports catalogs of up to 10,000 entries; this capacity is not evidence that 10,000 real theories have been curated or validated.

The discovery adapter queries Crossref for bibliographic candidates. It never promotes a title or DOI into a verified scientific claim. A curator must inspect the underlying source before creating a catalog entry. Only selected catalog context reaches the two AI judges; the full catalog remains in the local audit snapshot. See the [research guide](RESEARCH_GUIDE.md) for the workflow and limits.

## Claim-level evidence and final-score gating

Version 0.4 adds a separate checked evidence policy. Its claims map to every existing grading criterion and declare their conditions. The policy fixes source admission rules, a search plan and cutoff, independent-origin requirements, and bounded context. A prepared evidence input includes the task context, policy, and imported dossier before a learner answer is evaluated.

The dossier links claims to exact source-text quotations. OpenAI and Claude independently classify each link's relation, applicability, and eligibility. Rust derives supported, contradicted, conflicted, insufficient, or hypothesis states and records the blocked criteria. Declared shared origins and exact duplicate text cannot create extra independent support. Conflicting applicable evidence is preserved rather than settled by counting more copies.

```mermaid
flowchart TD
    A["Checked evidence policy and dossier"] --> B["OpenAI source review"]
    A --> C["Claude source review"]
    B --> D["Verified evidence pack"]
    C --> D
    D --> E["Answer evaluation under fixed anchors"]
    D --> F["Deterministic evidence gate"]
    E --> F
    F --> G["Settled awards or unresolved ranges"]
```

With `--evidence`, the pack is bound into the grading run and replay. The software gate withholds settled awards for blocked criteria even if both answer judges selected full credit. It preserves their original rows and removes affected final percentages. Missing source support does not become a learner's automatic zero. A supported source claim also does not replace the requirement to justify the chosen anchor using the learner's actual text.

This mode is opt-in to preserve existing workflows. Runs without `--evidence` do not apply source-audit gating. `--research` and `--evidence` are mutually exclusive so an unreviewed parallel pack cannot be silently added to the reviewed context. The evidence mode still uses exactly the original two providers.

An evidence-only revision compares packs for the same grading contract. A common plan freezes a declared cohort and marks every submission pending under the new evidence-policy version. It preserves old identities without executing new API calls or overwriting grades. The caller must still provide a complete cohort and carry out the reassessments. Changed grading criteria or points require a separate contract migration.

Source content, origin labels, search histories, and eligibility decisions remain fallible supplied information. Neither URL fields nor hashes authenticate a publisher; search records do not establish exhaustive retrieval. Both models can share a mistake. See the [evidence guide](EVIDENCE_GUIDE.md) for these boundaries and the synthetic protocol fixtures.

## Trust boundaries

Questions, answers, quotations, and source descriptions are data. They must not become operational instructions to change the contract. Model output is untrusted until the JSON shape, criterion set, anchor IDs, and required evidence have been checked. Invalid or incomplete responses fail closed rather than becoming a guessed grade.

Credentials are read from provider-specific environment variables. They are not placed in model prompts or audit files. Network requests use fixed provider endpoints, response-size limits, timeouts, output-token limits, and bounded retry attempts. These are operational bounds, not a promise of a particular monetary price.

Saved reports may contain the original question and answer, quotations, and provider responses. Keep private learner data out of public repositories. The default local run directory is ignored by Git.

## Version boundaries

The Python 0.1.1 implementation remains available as a historical prototype. It accepts manually supplied numerical judgments under its own JSON protocol. Rust 0.2.0 introduced the stricter anchor protocol and two-provider workflow. Rust 0.3.0 added optional research context while preserving the scoring rules. Rust 0.4.0 adds opt-in evidence auditing and final-score gates and preserves replay of 0.2 and 0.3 records without changing their original identities or reports. Neither Rust protocol silently accepts Python judgment files.

See [the Rust guide](RUST_GUIDE.md) for the executable contract, commands, and current file formats.

## Score display, regression gates, and finite proof checking

Version 0.6 caps official current/future percentages and their bounds at 99.9 after the ordinary weighted-anchor calculation. Coverage and raw anchor points remain unchanged. Replay selects the saved engine's score convention before evidence gating, so historical 0.2–0.5 records retain their original reports. Equal capped bounds do not settle an unresolved judgment.

`benchmark_gate` recomputes a full benchmark from a hash-bound suite and supplied trials, then checks frozen thresholds overall, for every declared split, and for every observed category. It rejects missing coverage, bad agreements, failed or indeterminate relations, and unmet category/repeat requirements according to policy. It does not authenticate imported API labels or establish that thresholds were chosen before seeing outcomes.

`proof` compiles a separate task-bound RelSpec challenge and checks a complete truth-table certificate by exhaustive recomputation. Its grammar, variables, formula depth, and assignment count are bounded. A proof result can add a cosmetic 10,000% celebration only after ordinary grading and evidence gates have also succeeded with full raw current anchors. The celebration never enters score arithmetic or benchmark metrics. The checker establishes only the specified classical Boolean entailment, not natural-language meaning, novelty, or arbitrary mathematics.

All new operations use the existing CLI, JSON service, local HTTP adapter, and JavaScript client. Challenge-author authorization and approval timing remain host responsibilities. See [historical analysis](HISTORICAL_FAILURES.md), [score policy](SCORE_POLICY.md), and [proof guide](PROOF_GUIDE.md).
