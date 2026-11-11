# Official grades, required human review, and the 99.9 ceiling

**A new AI result is not an official grade.** Rust 0.7 requires an examiner score and explicit review. Applications release only a validated, finalized `report.decision.final_percent`. The earlier `report.current.percent` field now represents the provisional AI current-task result. A `ready` AI status means its criteria are settled; it does not mean an examiner approved it.

## First calculate the AI profile

The compiler accepts predefined raw anchor points from 0 through 100. Criterion weights and critical thresholds use those raw points. Rust calculates the weighted result and applies:

```text
ai_percent = min(weighted_anchor_percent, 99.9)
```

The same cap applies to an AI profile's lower and upper bounds. This is a ceiling, not multiplication by 0.999. A raw 46.25 remains 46.25; raw 99.95 and 100 become 99.9. Coverage is different: it measures settled criterion weight and can still reach 100%. Benchmark accuracy and raw criterion points also retain their original units.

The 99.9 ceiling is the author's product convention. It is not a probability of truth, a confidence interval, or evidence that every answer contains exactly 0.1% uncertainty.

## Then require the examiner's decision

| Mode | AI weight | Examiner weight | Official current-task grade |
|---|---|---|---|
| `balanced` | 50% | 50% | `(ai_percent + human_percent) / 2` |
| `human_override` | 0% | 100% | `human_percent` |

The examiner must supply a finite score from 0 through 99.9, identify their grading basis, explain their judgment, and acknowledge reading the AI report and its limits. OpenAI and Claude constitute the existing reconciled AI side; there are not three equally weighted judges. Future-exploration scores never increase current-task credit.

**The examiner is free to reject the AI's direction and apply their own score in full.** Full human control is a decision weight, not a score of 100. The original AI findings, reasons, and prior human reviews remain available. An override changes an assessment decision; it does not establish factual truth or remove required integrity checks.

See [HUMAN_REVIEW.md](HUMAN_REVIEW.md) for the readable review page, CLI/API workflow, history, and host responsibilities.

## Uncertainty must survive display and review

The cap never settles a model disagreement. Two distinct unresolved raw outcomes can both display as 99.9 while the AI percentage remains null. Never infer agreement from equal rounded endpoints.

Balanced mode cannot finalize a missing or blocked AI result. It must not substitute zero, silently drop the AI side, or average interval endpoints. The examiner may investigate, or explicitly choose a reasoned full human override while preserving the original warnings. The final grade and retained AI status answer different questions.

When no validated human decision exists, display **“Examiner review required.”** Do not export a provisional AI number into a gradebook, rank students with it, or convert a missing grade to zero. Human reviewers can also make mistakes; retain correction and appeal procedures and apply changes consistently across an affected cohort.

## Historical replay

Rust 0.2–0.5 runs keep their original scoring scale, including possible 100 scores. Rust 0.6 introduced the 99.9 ceiling. These historical records retain their original engine semantics during replay and are not silently rewritten.

None of those historical formats establishes the new required examiner decision. For a current official grade, re-evaluate the original inputs under Rust 0.7 and complete its review workflow. The retained Python teaching prototype is historical and must not be used to bypass the human-review requirement.

## The separate 10,000% celebration

The optional [proof challenge](PROOF_GUIDE.md) can produce `celebration.display_percent: 10000`. It is a scoped visual celebration, not extra academic credit. For a new unreviewed proof result, `celebration.official_percent` is null because examiner review is still required.

Use only finalized `report.decision.final_percent` for official export, ranking, or grade calculations. Never use the celebration number or the provisional AI profile. The checked certificate does not approve the examiner's assessment, authenticate the author, or establish a novel mathematical discovery.

The badge requires full settled current-task raw anchors, no evidence or critical blocks, and a certificate checked against the locked finite propositional task. Merely reaching the capped 99.9 or claiming that AI proved a theorem is insufficient. Wherever the badge is shown, display its scope and the human-review state clearly.
