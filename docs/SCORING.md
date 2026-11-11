# Scoring rules

For positive weights w_i and scores s_i in [0,1], let K contain scored criteria, W be total weight, W_K scored weight, and E the sum of w_i*s_i over K.

- coverage = W_K / W.
- observed_percent = 100*E/W_K, or null when W_K is zero.
- lower_percent = 100*E/W.
- upper_percent = 100*(E + W - W_K)/W.

The bounds account only for unresolved scores, holding accepted scores fixed. They are not statistical confidence intervals.

| Status | Condition | Composite |
|---|---|---|
| blocked | A critical criterion is below its threshold | null |
| review | No scored criteria, an unresolved critical criterion, or insufficient coverage | null |
| ready | All configured aggregation requirements are met | observed_percent |

Status ready is permission to issue a summary under the policy, not proof that the judgments are correct. The default example coverage threshold of 0.8 is an illustrative policy choice.

The osmosis fixture has E=.75, W_K=.90, W=1. It yields observed fulfillment 83.33%, coverage 90%, and bounds 75–85%. Its deliberately null scope criterion demonstrates abstention; a required omitted assumption can instead receive zero in a real evaluation.

A perspective does not create a new share of credit. The current aggregator receives one resolved judgment per criterion. Semantic duplication across differently named criteria still requires rubric review.

See [textbook Chapter 4](../book/Percent-Grading-Textbook.md) for derivations and invariants.
