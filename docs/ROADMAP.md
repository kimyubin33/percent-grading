# Roadmap

## Version 0.1.1

- English code messages, prompts, examples, and technical documentation.
- English README and Korean README.
- English teaching text with exercises and worked answers.
- Explicit review state when nothing has been judged.
- All 16 local tests passed on Python 3.14.4 on 2026-11-11. GitHub Actions runs checks on pushes and pull requests.

## Next: AI adapter

Implement one optional API adapter, structured output, timeouts, bounded retries, cost limits, and raw-response audit records. Acceptance: fixed public examples can be reproduced with failure logs.

## Next: multiple perspectives

Load a sourced registry, select relevant lenses, group overlap, preserve competing judgments, and review or abstain on conflicts. Acceptance: publish comparisons at 1, 4 and 16 perspectives and a duplication stress test.

## Next: empirical study

Collect and independently review task data. Compare baselines under documented budgets. Publish accuracy, coverage, feedback quality and cost. Expand scale only after useful gains are demonstrated.
