# Required examiner review and human authority

**An AI report is advice for the examiner. It is not an official grade.** Starting with Rust 0.7.0, every newly calculated AI report has a pending examiner decision. The authorized examiner must supply their own score, read the AI report, and explicitly choose how the official decision is made. No model agreement, proof badge, API origin, or high percentage removes this requirement.

The examiner is free to reject the AI's assessment direction and apply their own assessment **100%**. This records a human assessment decision; it does not declare the examiner infallible or rewrite scientific facts. The original AI reasons and the history of human decisions remain available for review.

## The two permitted modes

Let `A` be the settled current-task AI score and `H` the examiner's independently supplied score. Both use the 0 through 99.9 scale.

| Mode | Official current-task result | Required interpretation |
|---|---|---|
| `balanced` | `0.5 * A + 0.5 * H` | Human 50%, AI 50% |
| `human_override` | `H` | Human 100%, AI 0%; the examiner's stated grading basis controls the assessment |

The AI side is the existing reconciled result of **OpenAI and Claude**, not a third equal vote alongside the examiner. Disagreement between those models is not secretly averaged away. Future-exploration points are separate and are not added to the current-task result.

If the AI report is unresolved or blocked, balanced mode cannot manufacture a number from missing evidence. The examiner can investigate and obtain a new supported report, or explicitly choose a reasoned human override. An override preserves the original AI warnings. It never bypasses valid input shapes, answer/task bindings, score limits, or required attribution and reasons.

**“100% human” is a decision weight, not a score of 100.** The official score ceiling remains 99.9. A 10,000% celebration badge never changes either input or the official result.

## Read the AI's evaluation as an explanation

Generate a standalone review page from the exact task, answer, and report:

```text
cargo run --locked -- review-report LOCK.json ANSWER.txt AI-REPORT.json --out local-runs/review.html
```

Open the HTML file in a browser. It presents the task, submitted answer, each criterion, the two models' brief reasons, quoted answer evidence, conditions, disagreements, and the human decision. This is the recorded, checkable justification, not a claim to expose a model's private internal reasoning. A `.md` output is available for plain-text reading.

The page offers a form for the examiner's score, balanced or override mode, grading basis, rationale, and acknowledgments. Its exported JSON is a **review request**. A browser preview is not an official decision: the Rust CLI or API must validate and apply it. The page does not need provider keys or contact an AI service.

To reduce anchoring, an examiner should first record an independent judgment where practical, then compare it with the AI's explanation. Do not turn required review into a routine “accept everything” button.

## Apply a review through the checked engine

```text
cargo run --locked -- review-template LOCK.json ANSWER.txt AI-REPORT.json --out local-runs/examiner.json
```

The template is intentionally incomplete. Set the examiner identifier, score, mode, grading basis, and rationale; read the report and set the two acknowledgment fields only when true. Preserve the generated report, task, answer, and expected-decision bindings. Then apply it:

```text
cargo run --locked -- review LOCK.json ANSWER.txt AI-REPORT.json local-runs/examiner.json --out local-runs/reviewed.json
cargo run --locked -- review-report LOCK.json ANSWER.txt local-runs/reviewed.json --out local-runs/reviewed.html
```

Output files are create-new. To revise a decision, generate a fresh template from the latest reviewed report and produce a new output file. The expected decision hash prevents a stale template from silently replacing a later revision within the supplied history. Prior reviews are retained in a bounded history; preserve the original files as well.

For applications, the corresponding operations are `review_template`, `review_report`, and `examiner_review`. See the [API guide](API_GUIDE.md).

## Which number an application may release

`report.current.percent` is the **provisional AI current-task score**. Its original `status` describes AI agreement and evidence conditions. Even `status: "ready"` does not authorize grade release.

Use the finalized `report.decision.final_percent` only after the decision is validated. If it is absent or null, show **“Examiner review required”**, not zero, a provisional rank, or an automatically approved score. Preserve the human and AI input scores, their weights, mode, reasons, history, and the remaining warnings beside the final result. Do not relabel an override as AI consensus.

Historical Rust 0.2–0.6 records remain replayable under their original rules. They do not contain the new required examiner decision and must not be silently promoted to approved 0.7 results. The Python teaching prototype is historical and must not serve as an official-grade shortcut. Re-evaluate the original inputs using the current engine before applying the new review workflow.

## Warnings that must remain visible

- **Human authority does not make either party error-free.** AI can misread reasoning, overlook context, cite weak evidence, or share the same mistake across models. Humans can also misjudge an answer. Keep explanations inspectable and provide a correction and appeal route.
- **An examiner identifier and acknowledgment are supplied statements.** Hashes detect changes and mismatched inputs; they do not authenticate a real person or establish that a page was read. A deployed host must authenticate reviewers and restrict who may approve or revise a grade.
- **Do not penalize style without an applicable rubric.** Grammar, fluency, unfamiliar phrasing, disability, language background, and stress are not proof of poor understanding or unauthorized AI use.
- **An override needs a defensible basis.** Record why the examiner's interpretation differs, the applicable task assumptions, and the resulting award. If the underlying rubric is wrong, version it and review the affected cohort consistently rather than quietly changing standards for one student.
- **Preserve uncertainty and provenance.** Required evidence, unresolved criteria, provider disagreement, manual fixtures, imported results, and live-provider records have different meanings. A human override does not turn an unsupported factual claim into established knowledge.
- **Keep assessment and discipline separate.** A grade, score difference, or AI-like writing style is not a misconduct finding. Any consequential integrity decision requires the institution's human process and a fair opportunity to respond.
- **Protect the record.** Review pages and JSON files can contain answers and examiner notes. Keep them out of public repositories and learner-facing bundles unless their specific disclosure is authorized.

These controls define software behavior. They do not establish educational validity, fair outcomes across groups, or measured detection effectiveness. Evaluate those questions separately and publish unfavorable as well as favorable results.
