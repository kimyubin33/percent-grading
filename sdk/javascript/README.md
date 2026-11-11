# JavaScript client

Dependency-free source client for the `percent-grading-api-v1` JSON service. This package is part of the repository and is not published to npm. Node.js 18 or later is supported through its built-in `fetch`; TypeScript declarations are included.

```javascript
import { createClient, PercentGradingError } from "./index.mjs";

const client = createClient({
  baseUrl: "http://127.0.0.1:8787",
  token: process.env.PERCENT_GRADING_API_TOKEN,
});
const { lock } = await client.compile(policySource);
const prepared = await client.prompt({ lock, answer });
```

The local token is a separate credential from `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`. Keep all credentials in native/backend processes. A web frontend should call its own authenticated backend; the Rust adapter rejects browser Origin headers and does not enable CORS.

Convenience methods are `compile`, `prompt`, `evaluate`, `grade`, `proofPolicy`, `proofPrompt`, `proofEvaluate`, and `benchmarkGate`. `request(payload, options)` also supports the evidence operations documented in the [API guide](../../docs/API_GUIDE.md). `grade` requires the service to be started with live calls explicitly enabled and can incur provider charges. Proof and gate methods run offline.

Official score profiles in engine 0.6 are capped at 99.9%; coverage and raw anchor points still use 0..100. `proofEvaluate` returns an official `report` and a separate nullable `celebration`. Show the latter's explanatory message alongside its 10,000% display; never use that display in averages, rankings or persisted official grades. The finite propositional checker is not an arbitrary mathematics verifier, and imported judgments do not attest provider execution.

```javascript
const { policy } = await client.proofPolicy({ lock, source: challengeSource });
const certificatePrompt = await client.proofPrompt({ lock, policy, answer });
// Supply a certificate matching certificatePrompt.schema and the two judgments.
const checked = await client.proofEvaluate({
  lock, policy, answer, certificate, openai: openaiJudgment, claude: claudeJudgment,
});
console.log("Official grade:", checked.report.current.percent);
if (checked.celebration) console.log(checked.celebration.message);
```

The host must approve and retain the challenge's mapping to the task before accepting answers. Do not let a student choose a simpler replacement challenge. Hash checks detect changes to a supplied lock; they do not authenticate instructor approval or establish the truth of natural-language claims.

`benchmarkGate({suite, results, policy})` returns `{gate}`. Inspect `gate.passed`: a failed gate is a successful API response with explicit violations, while malformed data throws a validation error. A passing gate only meets its declared checks; it does not establish educational validity or authorize a release.

Options include `baseUrl`, `token`, `timeoutMs` (default 180000), and an optional `fetchImpl`. Every call can receive `{requestId: "your-correlation-id"}`. The request identifier is not an experiment identity or a request to reroll a grading result.

`PercentGradingError` exposes `code`, `message`, and `status`. Transport errors have no HTTP status. Requests and responses are bounded to 2 MiB, redirects are rejected, and protocol/version/identity mismatches fail. There are no automatic retries. A timeout does not prove server processing has stopped.

```text
node --test sdk/javascript/test/client.test.mjs
node examples/integration/node-client.mjs
```

Run those commands from the repository root. Tests use mocked responses and no provider calls. The integration example needs a running local Rust service and only compiles a policy and prepares a prompt.
