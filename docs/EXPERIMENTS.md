# Experimental plan

No live benchmark results are claimed. Version 0.5 supplies an executable [benchmark workflow](BENCHMARK_GUIDE.md) and a [ten-task, sixty-case authored corpus](../benchmarks/README.md). Its manual result fixture demonstrates failure reporting; it is not measured model performance. The larger study below remains a research plan.

## Hypotheses

H1: fewer valid paraphrases are rejected than by an exact-match baseline. H2: reversed relationships are detected better than by a single overall AI score. H3: selected perspectives improve performance up to a measurable cost limit. H4: abstention reduces error at comparable coverage.

## Dataset design

An initial planning target is 60 tasks across life science, programming, and logic/mathematics. This is not a collected dataset or a justified final sample size. Each task should have valid, paraphrased, partial, reversed, missing-condition, irrelevant, and adversarial responses.

Split by underlying task to avoid variant leakage. Freeze rubric and selection policy before the held-out evaluation. Use multiple independent domain reviewers and retain disagreements.

## Comparisons and measures

Compare exact matching where appropriate, an ordinary analytic rubric, a single AI total, and the proposed profile. Report criterion error, critical-error precision/recall, false rejection, repeatability, coverage, latency, and cost. Check evidence quality separately from numeric agreement.

Assess abstaining systems at matched coverage. Respect task-level dependencies when estimating uncertainty. Stress test perspective duplication, answer order, irrelevant verbosity, and grading instructions embedded in answers.

Publish unfavorable results. If only feedback quality improves, restrict the claim to that outcome.
