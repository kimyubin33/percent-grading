# Claim-level evidence audits

Version 0.4 adds a checked evidence policy, claim-to-source links, two-provider source reviews, and a deterministic gate on final grading. It answers a narrower question than scientific truth: **does the supplied, reviewed material justify using this claim under the declared conditions?**

A DOI, a recognizable publisher, or agreement between two models does not establish truth. The new layer records how evidence was admitted, what it was said to support, and what remains unresolved. It does not crawl the web or certify an entire field's literature.

## Two contracts with different responsibilities

The grading contract fixes the task, criteria, anchors, and points. The evidence policy fixes the domain, inclusion and exclusion rules, search plan, cutoff, source limit, independence threshold, and context-byte budget. Both use the actual pinned RelSpec/AHAA compiler and additional Rust validation.

Evidence-policy claims name an existing grading criterion, a statement, its conditions, and its kind. Every criterion must have at least one claim. A hypothesis may map only to a future criterion. A policy and its claims are bound to the exact grading contract.

Prepare these inputs before inspecting learner answers. The program can verify bindings and structure; it cannot prove when a human first considered an answer. A source-based correction should create a new policy version and a common reassessment plan. Inclusion, exclusion, and cutoff are policy prose evaluated by the reviewers; dates are not automatically fetched or verified. For substantive use, include a source's date and version in its locator or supplied content so a cutoff can actually be assessed.

## Source material and claim links

A dossier contains imported source excerpts, links from claims to exact quotations, and declared support/counterevidence search records. The source fields preserve a title, URL, origin group, kind, locator, and supplied text. A link specifies the claim, source, quote, and location within that source.

The program checks that every linked quotation is an exact substring of the supplied source text. It does not fetch the URL to authenticate that text. A page URL or locator is provenance supplied by the curator, not a verified publisher signature. Source excerpts should be sufficient to assess the claim without stripping away important qualifications.

Use `origin_id` to group documents derived from the same original evidence. Reprints, press releases, and summaries of one experiment are not independent replications. Exact duplicate source text is also collapsed across origin labels. This is a guard against repetition, not a semantic detector of every shared dependency.

Task-reference sources have a special role: they can support intent claims using the exact frozen reference. They do not establish an empirical explanation by being copied from the expected answer.

Support and counterevidence search records are required. They document the submitted search process and outcome; the software does not attest that those searches were actually executed or exhaustive. A failed search is not proof that no counterexample exists.

## What the two reviewers return

OpenAI and Claude independently review the same fixed evidence input. Their categorical judgments cover every link exactly once:

| Field | Meaning |
|---|---|
| `relation` | The source supports, contradicts, or provides insufficient support for the claim |
| `applicable` | The source's conditions apply to the claim as stated |
| `quality` | The source is eligible, ineligible, or uncertain under the policy |
| `reason` | A short, checkable explanation for the classification |

They cannot assign trust percentages, invent a new claim, alter the evidence policy, or change rubric weights. Source text and quoted passages are untrusted data, not instructions. The code checks structure and provenance links; semantic eligibility and applicability remain judgments that both models can get wrong.

## Aggregated evidence states

| State | Interpretation |
|---|---|
| `supported` | The required independent support passed the review rules without unresolved or opposing applicable evidence |
| `contradicted` | Accepted applicable evidence contradicts the claim and there is no accepted supporting origin |
| `conflicted` | Accepted applicable support and contradiction coexist |
| `insufficient` | Support, applicability, eligibility, or agreement is not sufficient |
| `hypothesis` | The future claim passed the declared evidence requirements as a conditional hypothesis, not as an established present fact |

Opposing evidence is not averaged into a confidence score. Adding many copies of supporting material does not cancel a valid contradiction. Review disagreement or unresolved relevant evidence prevents a settled result under the conservative aggregation policy.

The evidence pack embeds its policy, dossier, both reviews, derived claim states, blocked criteria, and a content hash. Verification recomputes the derived results. These hashes detect inconsistent edits; an owner who rewrites the whole record can also recompute hashes. They are not external authenticity attestations.

## Offline demonstration

The bundled evidence dossier and reviews are **synthetic, manually authored protocol fixtures**. The example.org URLs do not identify scientific publications. Search outcomes say explicitly that no search was executed. These examples test validation, disagreement, and withholding behavior; they are not evidence of either model's accuracy or scientific source quality.

Run these commands from the repository root. If the grading lock already exists, reuse it rather than running the first command again. Every output uses create-new behavior and refuses to overwrite existing files.

```text
cargo run --locked -- compile policies/apple.aha --out local-runs/apple.lock.json
cargo run --locked -- evidence-policy local-runs/apple.lock.json policies/apple-evidence.aha --out local-runs/apple.evidence-policy.json
cargo run --locked -- evidence-prepare local-runs/apple.lock.json local-runs/apple.evidence-policy.json examples/evidence/apple.dossier.json --out local-runs/apple.evidence-input.json
cargo run --locked -- evidence-prompt local-runs/apple.lock.json local-runs/apple.evidence-input.json
cargo run --locked -- evidence-import local-runs/apple.lock.json local-runs/apple.evidence-input.json examples/evidence/openai.review.json examples/evidence/claude.review.json --out local-runs/apple.evidence.json
cargo run --locked -- inspect-evidence local-runs/apple.lock.json local-runs/apple.evidence.json
cargo run --locked -- prompt local-runs/apple.lock.json examples/rust/apple-blue.txt --evidence local-runs/apple.evidence.json
```

The review fixtures bind to the exact bundled contract, evidence policy, and dossier. Changing any of those inputs requires new reviews; changing only a hash in a review does not constitute a new assessment.

To inspect disagreement without calling an API:

```text
cargo run --locked -- evidence-import local-runs/apple.lock.json local-runs/apple.evidence-input.json examples/evidence/openai.review.json examples/evidence/claude.disagreement.review.json --out local-runs/apple.disputed-evidence.json
```

The ordinary manual pair leaves no blocked criteria and reports the future claim as `hypothesis`. The disagreement fixture marks the optical-basis claim `insufficient` and blocks `principle`. These are expected protocol outcomes of the authored classifications.

## Live evidence auditing and grading

Set `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` in the environment and select compatible model IDs explicitly. Replace the synthetic dossier with source text that you have permission to process and a real, documented search history before using the workflow for substantive evaluation.

```text
cargo run --locked -- audit-evidence local-runs/apple.lock.json local-runs/apple.evidence-input.json --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL --runs-dir local-runs/evidence
cargo run --locked -- replay-evidence AUDIT_RUN_DIRECTORY
cargo run --locked -- grade local-runs/apple.lock.json examples/rust/apple-blue.txt --evidence AUDIT_RUN_DIRECTORY/evidence.json --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL
```

`audit-evidence` makes the source-review calls. `grade` makes separate learner-answer evaluation calls. Both may incur charges. Both use only the existing official OpenAI and Claude adapters. Limits on tokens, response bytes, timeouts, and attempts are operational limits rather than a guaranteed monetary cap. An audit input with no source-claim links is rejected before credential checks or network calls; an empty offline import can still demonstrate insufficient evidence.

An audit saves its first provider responses, including responses later found invalid. Resume calls only a missing provider; it does not reroll an existing judgment until a preferred result appears. Replay validates saved inputs, raw responses, extracted content, and derived evidence without new provider calls.

Use either `--evidence` or the earlier `--research` option, not both. The evidence workflow deliberately excludes an unreviewed parallel research pack from its prompt. Plain grading and the 0.3 research workflow remain available for compatibility; using them does not claim that a source audit has occurred.

## Evidence gates the final score

The same evidence pack is supplied to both answer judges and included in the run identity. A criterion is blocked if any of its required claims remains unresolved under the evidence rules.

Rust enforces that block even if both answer judges choose full-credit anchors. The criterion receives no settled award, its allowed range is retained, and the affected profile has no final percentage. The original judgments remain visible for inspection. A source gap is not converted into a learner error or an automatic zero.

A supported reference claim does not automatically prove that an answer is correct. Exact learner-answer excerpts are still required, and the original anchor definitions still determine points. A future hypothesis can inform only the separate future profile.

## Common revision and reassessment

A new answer may expose a legitimate perspective missing from the original evidence policy. Review that perspective, create a new evidence-policy version, and produce a new evidence pack. The revision workflow compares two packs bound to the same grading contract, then enumerates every submission in a declared cohort for reassessment.

```text
cargo run --locked -- evidence-policy local-runs/apple.lock.json policies/apple-evidence-v2.aha --out local-runs/apple.evidence-policy-v2.json
cargo run --locked -- evidence-prepare local-runs/apple.lock.json local-runs/apple.evidence-policy-v2.json examples/evidence/apple-missing.dossier.json --out local-runs/apple.evidence-input-v2.json
cargo run --locked -- evidence-import local-runs/apple.lock.json local-runs/apple.evidence-input-v2.json examples/evidence/openai.missing.review.json examples/evidence/claude.missing.review.json --out local-runs/apple.evidence-v2.json
cargo run --locked -- evidence-compare local-runs/apple.lock.json local-runs/apple.evidence.json local-runs/apple.evidence-v2.json
cargo run --locked -- revise-evidence local-runs/apple.lock.json local-runs/apple.evidence.json local-runs/apple.evidence-v2.json examples/evidence/cohort.json --reason "Demonstrate common reassessment when a required reasoning link is missing." --out local-runs/apple.reassessment.json
```

This deliberately weakened revision omits the reasoning link while retaining the required claim. `reasoning` becomes blocked, and all three illustrative submissions in the cohort become pending. The example demonstrates a changed evidence basis; it is not a claim that deleting relevant evidence improves fairness.

The plan preserves old and new evidence identities, policy versions, the reason, and a frozen cohort. Every listed submission requires the same new evidence pack. Identical answers from different submission IDs remain distinct submissions.

The plan is a queue of pending work. It does not automatically grade, call providers, overwrite old records, or establish that the supplied cohort is complete against an external class roster. Run each affected answer under the new pack and retain the earlier result. A rubric change that alters criteria or points requires a new grading contract and a separately designed migration; this evidence-only revision command rejects cross-contract comparisons.

`evidence-compare` reports changes in claim definitions, evidence claim states, and blocked criteria. It does not measure learner-score sensitivity without actual answer judgments. Repeating controlled assessments under alternative eligible evidence packs remains necessary to study how source selection changes model decisions.

## What still requires evaluation

The checks establish reproducible software behavior. They do not prove source authenticity, search completeness, model independence, scientific truth, fairness across populations, or educational benefit. Audit false support, missed contradictions, source eligibility errors, shared model mistakes, and the rate of withheld grades using independently reviewed cases before relying on this system for consequential decisions.
