# JavaScript client

Dependency-free source client for the `percent-grading-api-v1` JSON service. This package is part of the repository and is not published to npm. Node.js 18 or later is supported through its built-in `fetch`; TypeScript declarations are included.

```javascript
import { createClient, officialPercent, PercentGradingError } from "./index.mjs";

const client = createClient({
  baseUrl: "http://127.0.0.1:8787",
  token: process.env.PERCENT_GRADING_API_TOKEN,
});
const { lock } = await client.compile(policySource);
const prepared = await client.prompt({ lock, answer });
```

The local token is a separate credential from `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`. Keep all credentials in native/backend processes. A web frontend should call its own authenticated backend; the Rust adapter rejects browser Origin headers and does not enable CORS.

Convenience methods include `compile`, `prompt`, `evaluate`, `grade`, `proofPolicy`, `proofPrompt`, `proofEvaluate`, `benchmarkGate`, `reviewTemplate`, `reviewReport`, and `examinerReview`. `request(payload, options)` also supports the evidence operations documented in the [API guide](../../docs/API_GUIDE.md). `grade` requires explicit host permission and can incur provider charges; it produces a provisional AI report. Review, proof, and gate methods make no provider calls.

**The examiner retains final authority.** Engine 0.7 requires an actual examiner score and documented review before releasing an official grade. The default is 50% examiner plus 50% AI; `human_override` applies 100% examiner weight. That weight does not mean a score of 100: the ceiling remains 99.9. `report.current.percent` is provisional AI output, even when its AI status is `ready`. Export only a finalized `report.decision.final_percent`; `officialPercent(report)` returns null for pending or legacy results and never falls back to the AI profile. It is a presentation helper, not cryptographic validation: the Rust service must validate records and the host must authorize reviewers.

AI reasons can be wrong, biased, or incomplete. Read the answer, rubric, evidence and both models' reasons before making a decision. A proof celebration is cosmetic; it cannot finalize a grade or enter averages. The finite propositional checker is not an arbitrary mathematics verifier. See [human review](../../docs/HUMAN_REVIEW.md) for revision history, review obligations, and human autonomy.

```javascript
const { policy } = await client.proofPolicy({ lock, source: challengeSource });
const certificatePrompt = await client.proofPrompt({ lock, policy, answer });
// Supply a certificate matching certificatePrompt.schema and the two judgments.
const checked = await client.proofEvaluate({
  lock, policy, answer, certificate, openai: openaiJudgment, claude: claudeJudgment,
});
console.log("AI proposal:", checked.report.current.percent);
console.log("Official grade (null until reviewed):", officialPercent(checked.report));
if (checked.celebration) console.log(checked.celebration.message);
```

Prepare an examiner form and readable HTML/Markdown with `reviewTemplate({lock, answer, report})` and `reviewReport({lock, answer, report})`. The template is intentionally invalid: its score and acknowledgements must be completed by the authorized examiner. Send the completed form as `examinerReview({lock, answer, report, review})`. Do not silently populate acknowledgements, copy the AI score into the examiner field, or add a frontend-only finalization path. Render HTML only in a trusted examiner context; it contains private answers and grading material. Revisions need a fresh template bound to the latest decision and preserve prior reviews.

The host must approve and retain the challenge's mapping to the task before accepting answers. Do not let a student choose a simpler replacement challenge. Hash checks detect changes to a supplied lock; they do not authenticate instructor approval or establish the truth of natural-language claims.

`benchmarkGate({suite, results, policy})` returns `{gate}`. Inspect `gate.passed`: a failed gate is a successful API response with explicit violations, while malformed data throws a validation error. A passing gate only meets its declared checks; it does not establish educational validity or authorize a release.

Options include `baseUrl`, `token`, `timeoutMs` (default 180000), and an optional `fetchImpl`. Every call can receive `{requestId: "your-correlation-id"}`. The request identifier is not an experiment identity or a request to reroll a grading result.

`PercentGradingError` exposes `code`, `message`, and `status`. Transport errors have no HTTP status. Requests and responses are bounded to 2 MiB, redirects are rejected, and protocol/version/identity mismatches fail. There are no automatic retries. A timeout does not prove server processing has stopped.

```text
node --test sdk/javascript/test/client.test.mjs
node examples/integration/node-client.mjs
```

Run those commands from the repository root. Tests use mocked responses and no provider calls. The integration example needs a running local Rust service and only compiles a policy and prepares a prompt.
