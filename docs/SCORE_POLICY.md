# Official scores and the 99.9 ceiling

Starting with Rust **0.6.0**, the highest official current-task or future-exploration percentage is **99.9**. This is a product convention requested by the project's author. It is not a measured probability of correctness, a confidence interval, or a claim that every answer contains exactly 0.1% uncertainty.

## Calculation

The compiler still accepts predefined anchor points from 0 through 100. Criterion weights and critical thresholds still use those raw anchor points. For each profile, Rust first calculates the ordinary weighted result and then applies:

```text
official_percent = min(weighted_anchor_percent, 99.9)
```

The same cap applies to the lower and upper bounds of an unresolved profile. It does not scale every score by 0.999.

| Raw weighted anchor result | Official profile percentage |
|---|---|
| 0 | 0 |
| 46.25 | 46.25 |
| 99.95 | 99.9 |
| 100 | 99.9 |

Coverage is a different quantity: it measures the weight of settled criteria and can still reach **100%**. Benchmark accuracy, relation success rates, and raw criterion points can also reach 100; they are not official learner grades.

## Uncertainty must survive display rounding

The cap never settles a disagreement. If two distinct raw outcomes both exceed 99.9, the displayed interval can become 99.9–99.9 while the final percentage remains `null`. Preserve `status`, criterion disagreements, and raw anchor bounds in the interface. Do not infer settlement from equal displayed endpoints.

Missing evidence remains unresolved rather than becoming zero. Critical failures remain visible even if another component earns a high score. Current fulfillment and future exploration remain separate profiles; do not add them together.

## Historical replay

Saved Rust 0.2, 0.3, 0.4, and 0.5 runs retain their original scoring semantics, including a possible official 100 and their original interpretation text. Replay computes using the saved engine version before applying any evidence gate. Changing only the version label on a new calculation would not be faithful replay.

New 0.6 runs use the new ceiling and a new engine identity. Earlier reports are not rewritten or silently regraded. The retained Python 0.1.1 teaching prototype also keeps its historical protocol and scale; applications should use the current Rust engine.

## The 10,000% discovery badge

The optional [proof challenge](PROOF_GUIDE.md) can return a separate `celebration.display_percent` of 10,000. The official result stays inside `report.current.percent` and never exceeds 99.9. Export, ranking, averaging, and benchmark logic must use the official report, never the celebration display.

The badge requires all current criteria to have their full raw anchors, a settled report without evidence or critical blocks, and a certificate independently checked against the locked finite propositional challenge. Merely reaching the capped 99.9, mentioning a famous theorem, or writing “the AI proved it” cannot unlock it. It is a deliberate visual joke, not an arithmetic overflow, runtime error, extra academic credit, or certification of a novel discovery.

The checked theorem's scope and the explanatory message must appear wherever an application displays the badge. Applications may hide the trigger as an Easter egg; they must not hide what the number means once it appears.
