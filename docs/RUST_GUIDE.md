# Rust implementation guide

**Rust 0.7 requires examiner review.** All newly generated AI reports are provisional. Read the report, enter the examiner score, and apply the 50:50 decision or a reasoned full human override before releasing `decision.final_percent`. See [HUMAN_REVIEW.md](HUMAN_REVIEW.md).

Version 0.2.0 adds a real Rust program, the pinned RelSpec/AHAA compiler, and exactly two live API adapters: OpenAI and Anthropic Claude. The Python 0.1.1 prototype remains available for historical exercises and has a different protocol.

The current release is 0.6.0. This guide explains the original grading workflow; the [research guide](RESEARCH_GUIDE.md) covers discovery and selection, the [evidence guide](EVIDENCE_GUIDE.md) covers source reviews and optional gates, and the [benchmark guide](BENCHMARK_GUIDE.md) and [API guide](API_GUIDE.md) cover experiments and application integration. New official scores are capped at 99.9; see [score policy](SCORE_POLICY.md) and the [checked proof example](PROOF_GUIDE.md). Older runs retain their original identities, score scale, and interpretation during replay.

## 1. Build and run without credentials

Use a current stable Rust toolchain with Cargo. Clone the repository and run the following commands from its root. The first build needs the dependencies listed in `Cargo.lock`; later builds can use Cargo's local cache.

```text
cargo build --locked
cargo run --locked -- compile policies/apple.aha --out local-runs/apple.lock.json
cargo run --locked -- inspect local-runs/apple.lock.json
cargo run --locked -- demo local-runs/apple.lock.json examples/rust/apple-red.fixture.json
cargo run --locked -- demo local-runs/apple.lock.json examples/rust/apple-blue.fixture.json
cargo run --locked -- demo local-runs/apple.lock.json examples/rust/apple-future.fixture.json
cargo run --locked -- demo local-runs/apple.lock.json examples/rust/apple-disagreement.fixture.json
cargo test --locked
```

`compile` refuses to overwrite an existing output file. Reuse the existing lock, or compile a changed policy to a new path. The examples require no API keys and make no network requests. Their judgments are manually authored fixtures; a fixture is not evidence that either provider produced that judgment.

After building, the executable is `target/debug/percent-grading` on Unix-like systems and `target\debug\percent-grading.exe` on Windows. Replace `cargo run --locked --` with that executable path if preferred.

## 2. What each command does

| Command | Purpose | Makes API calls? |
|---|---|---|
| `compile SOURCE.aha --out LOCK.json` | Check the source and create a new frozen contract | No |
| `inspect LOCK.json` | Verify and display the locked contract | No |
| `prompt LOCK.json ANSWER.txt` | Generate the evaluator request for inspection | No |
| `demo LOCK.json FIXTURE.json` | Apply a teaching fixture to the checked contract | No |
| `evaluate LOCK.json ANSWER.txt OPENAI.json CLAUDE.json` | Validate and reconcile two supplied judgment objects | No |
| `grade LOCK.json ANSWER.txt --openai-model MODEL --claude-model MODEL` | Obtain and record judgments from both supported providers | Yes, unless reusing a valid saved run |
| `replay RUN_DIRECTORY` | Verify and reproduce a saved run | No |

The two `evaluate` input filenames identify their roles in reconciliation. This offline command cannot independently authenticate who authored those files. Use the live adapter's preserved responses when provider provenance matters.

## 3. The real RelSpec boundary

The user's [RelSpec repository](https://github.com/kimyubin33/relspec-lang) is now named AHAA. This project includes its actual compiler source at commit `de1e956555dcfafc5b62324ea6ed3f62655c6ba3`. See [the provenance record](../vendor/relspec/UPSTREAM.md).

AHAA uses `낱말` for a word declaration, `장면` for a scene, `*` for a required dimension, and `(정수)` for an integer type. These native keywords remain Korean. Identifiers and descriptions in this project's policies are English.

Percent Grading permits word declarations and exactly one flat scene. It rejects nested scene items, unspecified dimensions, and constructs outside this grading subset. It then validates the checked scene records against a closed schema. AHAA's scene fill percentage is not used as an answer grade.

### Record schema

Each record has a globally unique ASCII identifier of 1–64 letters, digits, underscores, or hyphens. `unresolved` is reserved for judgments. Field names must match the record schema exactly.

| Record type | Required fields | Main constraints |
|---|---|---|
| `Policy` | `version`, `question`, `context`, `reference` | Exactly one policy |
| `Criterion` | `dimension`, `weight`, `description`, `critical_min` | 1–100 criteria; positive integer weights from 1 to 100 |
| `Anchor` | `criterion`, `points`, `description` | Existing criterion; integer points from 0 to 100 |
| `Perspective` | `criteria`, `kind`, `source`, `assumptions`, `description`, `overlap` | Existing criteria; 1–256 declared perspectives |

The supported dimensions are `intent`, `principle`, `reasoning`, `conditions`, and `future`. At least one intent criterion is required. Current-task criterion weights sum to 100. If future criteria are present, their weights separately sum to 100.

Every criterion has 2–8 anchors with distinct point values, including 0 and 100. Every criterion must be covered by at least one perspective. Perspective `criteria` is a whitespace-separated list of criterion IDs, and `kind` is `established`, `method`, or `hypothesis`. A source field records provenance; the compiler does not fetch or verify that source's claims. The overlap field groups related lenses without awarding extra weight.

The example source label `RustGuide` resolves to this document. For the optics perspective, its background references are NASA's [Wave Behaviors](https://science.nasa.gov/ems/03_behaviors/) and the University of Kentucky's [Physics of Light: Color](https://www.pa.uky.edu/sciworks/light/preview/color4aa.htm). For the task-alignment and conditional-design perspectives, it refers to the explicit methods described here, not an independently validated theory. AHAA free-text fields have their own lexical rules; do not insert an unquoted URL containing slash characters into a scene value. A declared source label keeps the source text valid without implying automatic retrieval.

`critical_min` is `none` or an integer from 0 to 100. If both judges agree on an anchor below that threshold, the report is blocked. Future criteria cannot be critical gates for present-task success.

### A minimal color-only contract

This complete example asks only for identification. It does not secretly require an explanation. Save it as a new `.aha` file if trying it; the repository's apple policy deliberately asks a richer question.

```text
낱말 Policy: version*, question*, context*, reference*
낱말 Criterion: dimension*, weight*(정수), description*, critical_min*
낱말 Anchor: criterion*, points*(정수), description*
낱말 Perspective: criteria*, kind*, source*, assumptions*, description*, overlap*

장면 ColorOnly
  Policy color_only: version 1, question Name the observed color of the specified apple, context An apple with red skin under broad white illumination, reference Red
  Criterion color: dimension intent, weight 100, description Identify the color in the declared context, critical_min 100
  Anchor color_wrong: criterion color, points 0, description Does not identify red in the declared context
  Anchor color_red: criterion color, points 100, description Identifies red in the declared context
  Perspective visual_color: criteria color, kind method, source RustGuide, assumptions Use only the declared specimen and illumination, description Inspect the stated color, overlap direct_color
```

The checked lock includes both the original AHAA source and its compiled contract. Loading it recompiles the source and checks the resulting content and identity. Editing a weight only in the lock's JSON cannot bypass the language contract. This is an integrity check, not a cryptographic signature identifying who authorized a policy.

## 4. The richer apple policy

The bundled policy explicitly asks for the perceived color, a physical explanation, and relevant conditions. It names a red-skinned specimen in broad white illumination. This avoids treating every apple or every viewing condition as identical.

| Dimension | Current weight | Why it is separate |
|---|---:|---|
| Intent | 25 | Does the conclusion answer the specified task? |
| Principle | 30 | Are the relevant physical ideas valid? |
| Reasoning | 25 | Do those ideas support this conclusion? |
| Conditions | 20 | Are the assumptions and changed scenarios explicit? |
| Future | Separate weight 100 | Is a proposed possibility explicit and testable? |

These are declared teaching weights, not empirically validated educational measurements. A color-only task can use the smaller policy above. An explanation task must tell the learner that explanations are evaluated before collecting answers.

The blue fixture demonstrates partial recognition without accepting its conclusion. The future fixture shows that a proposed alteration or experiment can be evaluated separately. The disagreement fixture shows why two differing anchors do not become an average score.

## 5. Judge output: anchors, not numbers

Each provider must return one JSON object with exactly these fields:

```json
{
  "contract_hash": "COPY_FROM_REQUEST",
  "answer_hash": "COPY_FROM_REQUEST",
  "judgments": [
    {
      "criterion_id": "color",
      "anchor_id": "color_red",
      "evidence": ["Red"],
      "reason": "The answer identifies the specified color.",
      "conditions": ["The declared specimen and illumination apply."]
    }
  ]
}
```

This is the shape for the one-criterion teaching contract, not a reusable judgment for every answer. Real hashes must come from the generated request. Every criterion must occur exactly once, each anchor must belong to that criterion, and quotes must be exact nonempty answer substrings. A positive anchor requires at least one quote. Reasons are nonempty; evidence and conditions each have a maximum of 16 entries.

Use the literal anchor ID `unresolved` when a judgment cannot be justified. A clearly absent requirement may receive its zero anchor; absence does not automatically mean unresolved. Extra fields such as `score` or `points` are rejected. Provider structured output reduces format errors, and Rust validates the result again.

Checking that “reflection” occurs in an answer does not prove that its blue conclusion follows. Evidence validation is necessary but not sufficient for semantic accuracy.

## 6. Read the report

`current` contains the present-task profile. `future` contains a separate profile when future criteria exist; otherwise it is null. There is no combined present-plus-future score.

| Field | Interpretation |
|---|---|
| `percent` | Fixed weighted score only when every criterion in that profile has an agreed anchor; otherwise null |
| `coverage_percent` | Share of profile weight with agreed anchors |
| `lower_percent`, `upper_percent` | Weighted bounds from agreed, differing, or unresolved anchor points |
| `criteria` | Both raw validated judgments and the resulting criterion status |
| `disagreements` | Criterion IDs for which the two anchor IDs differ |
| `critical_failures` | Agreed outcomes below a declared critical minimum |

An agreed anchor has a single point value. Two different concrete anchors yield the interval between their point values. If either judge is unresolved, that criterion's interval is 0–100. Bounds are arithmetic descriptions of these possibilities, not a calibrated uncertainty estimate about the answer's truth.

The overall status is `ready` when all criteria agree and no critical gate fails. It is `review` when an outcome remains unresolved or disputed. It is `blocked` when an agreed critical failure exists, even if other criteria deserve credit. A blocked report preserves numerical profiles where agreement is complete; those numbers do not declare task success.

Future disagreement does not erase an agreed current profile, although the overall run still requires review. Likewise, a perfect future score cannot raise current fulfillment.

## 7. Configure the two live providers

Create API credentials through each provider's official account interface. Set `OPENAI_API_KEY` for OpenAI and `ANTHROPIC_API_KEY` for Claude in the process environment. The CLI does not automatically load a `.env` file. Do not paste keys into a policy, answer, issue, source file, or terminal transcript that you intend to share.

Supply explicit model IDs available to your accounts. The OpenAI model must support the Responses API with the requested structured JSON output. The Claude model must support structured outputs in the Messages API. The CLI does not silently switch models or relax the schema when a model rejects the request.

```text
cargo run --locked -- grade local-runs/apple.lock.json examples/rust/apple-blue.txt --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL --runs-dir local-runs --max-output-tokens 4096 --max-attempts 1 --timeout-seconds 60
```

Only the fixed official OpenAI and Anthropic endpoints are used. There is no generic provider URL or third-provider option. Redirects are disabled. The adapters reject refusal, truncation, unsupported response content, malformed JSON, or an invalid judgment rather than turning it into a score.

`max-attempts` includes the first request and is limited to 1–3 per provider. Retries are limited to transient failures. A timeout does not prove that a provider did not process or bill a request, so automatic retries are bounded. Output tokens and timeouts constrain each request, not its exact price. Consult your provider account for actual model pricing and usage.

Implementation references: [OpenAI Responses API](https://platform.openai.com/docs/api-reference/responses) and [Anthropic structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs). API support can change; an unsupported model/configuration produces an explicit failure.

## 8. Records and replay

The live workflow derives an identity from the fixed evaluation inputs, including the checked contract, exact answer, configured models, engine/prompt versions, and request limits. It preserves the input and provider evidence in a local run directory. A valid existing result is reused instead of silently asking the models to make a fresh decision about the same run.

| File in a live run | Contents |
|---|---|
| `input.json` | Checked contract source, compiled contract, exact answer, and run identity |
| `openai.response.json` | OpenAI response, extracted text, usage, and request metadata |
| `claude.response.json` | Claude response, extracted text, usage, and request metadata |
| `report.json` | Deterministically computed report after both responses validate |
| Provider error files | Failure information when a request does not produce an accepted result |
| `running.lock` | Temporary coordination file that prevents simultaneous runs from racing |

If the first provider succeeds and the second fails, a later invocation reuses the preserved first response and requests only the missing provider. A returned but invalid judgment is preserved and rejected again; it is not silently replaced by another attempt until a favorable judgment appears. A crash may leave `running.lock`; inspect whether another process is active before treating the lock as stale. Incomplete files or inconsistent saved records cause an error rather than automatic overwriting.

Use the directory reported by the live command:

```text
cargo run --locked -- replay local-runs/RUN_ID
```

Replay verifies the saved material and recomputes the deterministic report without API calls. Changing the policy, answer, or model configuration creates a different evaluation identity. Even whitespace changes to the exact source or answer can change that identity. A cache is not proof of truth; it preserves the original judgment. A mutable provider model alias may behave differently on a future fresh request even if its string is unchanged.

The run files are local records, not digitally signed provider attestations. Consistency checks detect mismatched fields or a modified report; they cannot prove that a person with write access did not construct an entirely new, internally consistent record.

Treat saved records as sensitive when they contain private answers. They can contain the full input, quotes, and model response. The local run directory is ignored by Git. No API key is intentionally written into a record.

## 9. Validation and limits

Tests cover contract structure, reference integrity, anchor membership, evidence checks, disagreements, critical failures, future separation, provider request/response handling, and replay behavior. Test fixtures do not make paid model calls.

The language checker cannot establish scientific truth in a free-text anchor. Two providers can agree on the same mistake. Prompt instructions and JSON validation cannot prove resistance to every injection attempt. Version 0.3 adds bibliographic discovery and automatic selection from declared catalogs, described in the [research guide](RESEARCH_GUIDE.md). It does not automatically verify scientific claims or adjudicate a genuine disagreement. These limits are explicit so the next experiments can measure them.

For the design rationale, read [Architecture](ARCHITECTURE.md) and [textbook chapter 14](../book/Percent-Grading-Textbook.md#14-a-strict-language-contract-and-two-ai-judges). For planned validation, read [Experiments](EXPERIMENTS.md) and [Roadmap](ROADMAP.md).
