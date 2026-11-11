# A checked proof and a playful 10,000% badge

Rust 0.6 includes a working, deliberately small proof-certificate checker. A task author freezes a propositional-logic challenge through RelSpec/AHAA. A learner can ask OpenAI or Claude to help construct a truth table, then submit the answer and certificate. Rust independently recalculates the table. An AI's opinion is not the verifier.

The optional result is a **celebration**, not a higher official grade:

```json
{
  "display_percent": 10000,
  "official_percent": null,
  "message": "Grading overflow unlocked! 10,000% is a celebration badge. The AI score is 99.9%; the official grade is pending examiner review. Verified only for this finite propositional challenge; this is not a software error or a general mathematical proof."
}
```

Starting with Rust 0.7, the badge does not approve a grade. Applications must display the review state alongside the badge and use only a validated finalized `report.decision.final_percent` for official exports, ranking, and grade calculations. `report.current.percent` is the provisional AI result; the examiner must supply their score and choose 50:50 review or full human override. See [HUMAN_REVIEW.md](HUMAN_REVIEW.md). There is no integer overflow or intentional crash. A valid certificate can still return `celebration: null` when the ordinary grading result is incomplete or blocked.

## Run the complete offline example

From the repository root:

```text
cargo build --locked
cargo run --locked -- compile examples/proof/grading.aha --out local-runs/proof-task.json
cargo run --locked -- proof-policy local-runs/proof-task.json examples/proof/challenge.aha --out local-runs/proof-policy.json
cargo run --locked -- proof-prompt local-runs/proof-task.json local-runs/proof-policy.json examples/proof/answer.txt
cargo run --locked -- proof-evaluate local-runs/proof-task.json local-runs/proof-policy.json examples/proof/answer.txt examples/proof/certificate.json examples/proof/judgment.json examples/proof/judgment.json
```

The example uses the same **manually authored judgment fixture** for both provider roles to demonstrate the protocol. It does not assert that either model produced it. The certificate is also an authored fixture. These commands make no network or paid API calls. Output files are create-new; reuse an existing compiled file or choose a fresh path when repeating a command.

The final command returns `origin: "imported_judgments_and_certificate"`, the ordinary `report`, a scoped `verification`, `evidence_hash` (null without evidence), and an optional `celebration`. The report is recomputed from the locked task and judgments; a caller cannot supply a claimed score of 99.9 to unlock the badge. Add `--evidence EVIDENCE.json` to apply the same checked evidence gate used by ordinary grading.

`proof-prompt` returns `system`, `user`, and `schema` for preparing a certificate with OpenAI or Claude. It does **not** automatically request or pay for a proof. Save the resulting structured certificate and pass it to `proof-evaluate`. Ordinary answer judgments can be imported or obtained through the existing two-provider grading workflow. Neither the certificate nor an imported provider label authenticates its author.

## What the example proves

The frozen variables are `P Q`, the premises are `P + P -> Q`, and the conclusion is `Q`. Here `+` separates premises; it is not arithmetic addition or a formula operator. This is modus ponens. The exact formal assertion included in the answer is:

```text
((P) AND (P -> Q)) -> (Q)
```

Bit zero of `assignment` represents `P`; bit one represents `Q`.

| Assignment | P | Q | Premise P | Premise P -> Q | Conclusion Q |
|---|---|---|---|---|---|
| 0 | false | false | false | true | false |
| 1 | true | false | true | false | false |
| 2 | false | true | false | true | true |
| 3 | true | true | true | true | true |

Only assignment 3 satisfies both premises, and its conclusion is true. Every other assignment still has to be supplied and checked; it cannot be removed from the certificate. A false conclusion where all premises hold rejects the certificate. Premises that cannot be satisfied at all also cannot unlock the badge, even though classical entailment from inconsistent premises is vacuously valid.

## Authoring a challenge

The separate challenge source defines exactly one `ProofChallenge` record with these fields:

| Field | Meaning |
|---|---|
| `version` | Author's nonempty challenge version |
| `criterion` | Existing current-task criterion; a future criterion is rejected |
| `variables` | 1–8 unique space-separated ASCII names, starting with a letter |
| `premises` | Up to eight unique formulas separated by `+`, or `none` |
| `conclusion` | The proposition to check whenever all premises hold |

Use [challenge.aha](../examples/proof/challenge.aha) as the executable syntax reference. Variables have at most 16 characters and may contain ASCII letters, digits, and underscores; the reserved operators `NOT`, `AND`, and `OR` cannot be variable names. Every declared variable must occur in the challenge. Operators, from highest to lowest precedence, are `NOT`, `AND`, `OR`, and `->`; implication associates to the right. Parentheses specify grouping. A formula is limited to 1,024 bytes, 256 tokens, and nesting/expression depth 32. The policy source is limited to 32 KiB and 128 lines. The word operators and `+` premise separator are compatible with the actual pinned RelSpec/AHAA lexer; the upstream compiler is unchanged.

The checker supports classical two-valued propositional logic. It does not execute scripts, infer unstated premises, quantify over integers, run an external solver, or accept a natural-language proof as a substitute for a table. The exhaustive domain contains at most 256 assignments.

The host application must decide who may create and approve challenges, then freeze the intended challenge before accepting learner answers. A contract hash prevents accidental mixing of different tasks; it does not prove that a trusted instructor approved the challenge or that approval happened first. A learner allowed to redefine the challenge could select a trivial theorem. The local API is not an instructor/learner authorization system.

## Certificate and award conditions

A certificate contains `format`, `policy_hash`, `answer_hash`, `answer_excerpt`, and `rows`. Each row contains `assignment`, the ordered `premises` Boolean values, and the `conclusion` Boolean value. See [certificate.json](../examples/proof/certificate.json) for a complete example.

Rust checks the source-derived policy, its task binding, the answer hash, the exact formal assertion in the submitted answer, complete assignment coverage, and every calculated truth value. Unknown fields, duplicate or missing assignments, wrong task bindings, counterexamples, and inconsistent premises are rejected. The assertion's occurrence establishes a textual link; it does not prove that the surrounding natural-language explanation is sound.

To return a celebration, the recomputed ordinary report must be ready, contain no disagreement or critical failure, and award **raw 100 on every current criterion**. Its provisional AI current score must be 99.9; the official score remains pending until examiner review. Evidence gates remain effective. Future exploration cannot compensate for missing current credit. A raw score just below 100 that becomes 99.9 after capping is insufficient.

The report and certificate remain separate: a complete truth table does not award missing explanation points, and two agreeing high-scoring judgments do not establish a formal entailment.

## Why this is not a Fermat verifier

Fermat's Last Theorem illustrates the desired surprise, but it is already a proved theorem; see [Wiles's 1995 paper](https://annals.math.princeton.edu/1995/141-3/p01). This release does not check that proof, certify a new discovery, or calculate that a proof is “99.9% complete.” The 99.9 ceiling is the project's scoring convention.

A future arithmetic or proof-assistant integration would need its own trusted checker, formal statement, allowed axioms, dependency provenance, and replayable certificate. It must identify exactly what was established and treat unsupported or unfinished verification as such. Until then, mentioning Fermat, claiming that an AI proved a theorem, or submitting a persuasive narrative cannot unlock this badge.

See [the score policy](SCORE_POLICY.md), [historical failure analysis](HISTORICAL_FAILURES.md), and [application API guide](API_GUIDE.md) for the surrounding boundaries.
