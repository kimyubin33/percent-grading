# Embedding Percent Grading in applications

Version 0.5 exposes the existing Rust engine through a versioned JSON service and a loopback HTTP adapter. A desktop application, native service, or web backend can use the same compilation, evidence, and scoring code as the CLI. The adapter is a local integration component, not a hosted multi-user grading product.

## Choose an integration boundary

| Caller | Suggested interface | Responsibility of the host application |
|---|---|---|
| Rust application | `percent_grading::service::dispatch` or the typed core modules | Supply trusted storage/configuration and present review states faithfully |
| Another native language | JSON over the CLI `api` command or local HTTP | Preserve the complete versioned request/response; handle process/network failures |
| Node.js backend | The dependency-free JavaScript client | Keep the local service token and provider credentials on the server |
| Browser or mobile frontend | Its own authenticated application backend, which calls this service | Establish user identity, task permissions, persistence, and application-specific limits |

The learner-facing frontend should receive the relevant result and explanations from its backend. The Rust adapter rejects browser `Origin` requests and provides no CORS access. Do not place OpenAI, Claude, or local service bearer credentials in a public frontend bundle.

## Rust library

The repository already builds a Rust library as well as the executable. Version 0.5 adds a transport-independent `service` module. Its JSON envelope can stay consistent while an application chooses a different transport.

```text
cargo run --locked --example embed
```

The [embedding example](../examples/embed.rs) compiles the bundled apple contract with `ServiceContext::default()`. It does not start a server or call a provider. Core types remain available from modules such as `contract`, `grading`, `evidence`, and `benchmark`.

`dispatch` is synchronous. Async application servers should run its blocking work on an appropriate blocking executor. Live provider requests use the existing bounded adapters and preserve the existing run records.

## JSON request and response

All requests use `api_version: "percent-grading-api-v1"`, a request identifier, and a tagged operation payload. Unknown fields and operations are rejected. The identifier correlates a response with a request; it does not alter scoring or create a fresh grading trial.

```json
{
  "api_version": "percent-grading-api-v1",
  "request_id": "compile-example",
  "payload": {
    "operation": "compile",
    "source": "THE COMPLETE AHAA POLICY SOURCE"
  }
}
```

The placeholder above is explanatory text. The repository includes a complete executable [compile request](../examples/integration/compile.request.json):

```text
cargo run --locked -- api examples/integration/compile.request.json
```

A successful compile returns a `result` object containing `lock`. Errors return `error` with `code` and `message` instead of `result`. Responses always include the API version. A malformed request or an HTTP rejection before parsing has `request_id: null`.

```json
{
  "api_version": "percent-grading-api-v1",
  "request_id": "grade-example",
  "error": {
    "code": "live_disabled",
    "message": "Live grading is disabled."
  }
}
```

Messages may become more specific without changing the error code. Integrations should branch on the code, not match the English sentence. Treat result formats as versioned data; future incompatible protocols require an explicit API-version change.

## Operations

| Operation | Payload fields besides `operation` | Successful `result` |
|---|---|---|
| `compile` | `source` | `lock` |
| `prompt` | `lock`, `answer`, optional `evidence` | `system`, `user`, `schema` |
| `evaluate` | `lock`, `answer`, `openai`, `claude`, optional `evidence` | Imported-judgment origin, evidence metadata, and `report` |
| `grade` | `lock`, `answer`, `config`, optional `evidence` | Saved grading result with the local filesystem path removed |
| `evidence_policy` | `lock`, `source` | `policy` |
| `evidence_prepare` | `lock`, `policy`, `dossier` | `input` |
| `evidence_aggregate` | `lock`, `input`, `openai`, `claude` | `pack` and imported-review origin |
| `proof_policy` | `lock`, `source` | `policy`, a task-bound checked proof challenge |
| `proof_prompt` | `lock`, `policy`, `answer` | Offline certificate `system`, `user`, and `schema` |
| `proof_evaluate` | `lock`, `policy`, `answer`, `certificate`, `openai`, `claude`, optional `evidence` | Recomputed official `report`, scoped `verification`, optional `celebration` |
| `benchmark_gate` | `suite`, `results`, `policy` | `gate`, a recomputed report with `passed`, `violations`, and the underlying benchmark |

`lock` and `evidence` are complete checked objects, not filenames. Every operation revalidates their applicable bindings. `openai` and `claude` in `evaluate` are complete grading judgments; in `evidence_aggregate` they are complete source-review records. Imported records do not establish that either provider was called.


The four new 0.6 operations are offline and do not require `allow_live`. A failed benchmark gate is a valid evaluation result (`200` with `result.gate.passed: false`), not a transport error. The CLI `benchmark-gate` saves its report and returns exit code **3** for a failed gate; malformed inputs return **2**. An invalid proof certificate returns `validation_failed`; a valid certificate with incomplete ordinary grading returns no celebration. See the [proof guide](PROOF_GUIDE.md) and [adversarial corpus](../benchmarks/adversarial/README.md).

`grade.config` uses the same `RunConfig` as the Rust runner:

```json
{
  "openai_model": "YOUR_OPENAI_MODEL",
  "claude_model": "YOUR_CLAUDE_MODEL",
  "limits": {
    "timeout_seconds": 60,
    "max_output_tokens": 4096,
    "max_response_bytes": 2097152,
    "max_attempts": 1
  }
}
```

The API does not accept provider keys, arbitrary provider URLs, client-chosen filesystem paths, or a third provider. Live source auditing and experiment scheduling remain available through their CLI/library interfaces; the HTTP surface does not expose every CLI operation.

## Local HTTP adapter

Set `PERCENT_GRADING_API_TOKEN` to a private randomly generated value containing 32 to 256 visible ASCII characters with no whitespace. The length check does not establish entropy. For example, Node.js can generate a value with `node -p "require('node:crypto').randomBytes(32).toString('hex')"`; keep its output private and assign it to the environment variable. This is a local service credential, separate from the provider credentials. Then start:

```text
cargo run --locked -- serve --bind 127.0.0.1:8787
```

The default service context has live calls disabled. It supports offline compilation, prompt preparation, imported-judgment evaluation, and evidence preparation. Only an explicit `--allow-live true` enables the `grade` operation:

```text
cargo run --locked -- serve --bind 127.0.0.1:8787 --allow-live true --runs-dir local-runs/api
```

Live grading also requires `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` in the service process environment. The JSON `api` command accepts the same `--allow-live` and `--runs-dir` options. Enabling live calls can incur provider charges; it does not start a request by itself.

| Route | Behavior |
|---|---|
| `GET /health` | Unauthenticated API/engine-version and status check; no provider call |
| `POST /v1/dispatch` | Authenticated versioned JSON operation |

POST requests require `Content-Type: application/json` and `Authorization: Bearer YOUR_LOCAL_SERVICE_TOKEN`. The listener accepts loopback bind addresses only, validates the Host header, rejects browser Origin headers, bounds request size, and limits request handling. Use an application backend for browser access; do not remove these restrictions just to expose the adapter publicly.

The request limit is 2 MiB. Errors include malformed requests (`400`), missing authentication (`401`), disabled live operations or rejected origins (`403`), unknown routes (`404`), unsupported methods (`405`), oversized payloads (`413`), unsupported media types (`415`), invalid evaluation data (`422`), busy handling (`429`), and provider failure (`502`). Timeout and internal failures also return structured errors. The HTTP status supplements the JSON error code.

## JavaScript client

The source client is included under [sdk/javascript](../sdk/javascript/README.md). It has no npm dependencies and is not published to npm. It requires a modern `fetch` implementation; Node.js 18 or later provides one.

With the local server running and the same service token in the client process environment:

```text
node examples/integration/node-client.mjs
```

This example compiles the apple policy and prepares a prompt without calling AI APIs. The client has TypeScript declarations, validates response version and request identity, bounds response size, and reports structured errors. Its response limit is 2 MiB; a larger otherwise-valid result is rejected rather than truncated. Large integrations can call the Rust or JSON CLI interfaces directly.

```javascript
import { createClient } from "./sdk/javascript/index.mjs";

const client = createClient({
  baseUrl: "http://127.0.0.1:8787",
  token: process.env.PERCENT_GRADING_API_TOKEN,
});
const { lock } = await client.compile(policySource);
const prompt = await client.prompt({ lock, answer: "The supplied answer." });
```

There are no automatic retries. A client timeout does not prove that the server stopped processing. Reusing the same grading inputs normally refers to the same saved run, but callers should inspect the resulting record instead of requesting a preferred new judgment.

Run the client tests with:

```text
node --test sdk/javascript/test/client.test.mjs
```

## Application responsibilities

The application decides who may submit answers, author rubrics, select evidence, and read reports. This adapter does not implement accounts, classroom membership, per-user billing, a durable job queue, or production deployment. It does not validate arbitrary source authenticity or make the two judges semantically independent.

Keep the full profile and review states when designing a UI. A `null` final percentage means that the profile is unresolved, not that the learner scored zero. Future exploration remains separate from present correctness. Preserve the policy and evidence identities when exporting results so a later revision can be traced.

New Rust 0.6 official grades and score bounds are capped at **99.9**. Coverage, benchmark accuracy, and raw anchor points retain their original units. An optional `celebration.display_percent` of **10,000** is cosmetic: never substitute it for the official score, and show its explanatory message. Capping can make unresolved interval endpoints equal; `status` and a `null` final score still require review. Historical saved runs retain their original scale. See [score policy](SCORE_POLICY.md).

The host must authorize proof-challenge authors and freeze their approved mappings before accepting answers. The service validates hashes and formal entailment, not instructor identity, chronology, natural-language formalization, or novel discoveries. Gate policies likewise require host review; a hash binds a threshold choice without proving it was chosen before seeing results.
