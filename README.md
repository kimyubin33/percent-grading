# Percent Grading

**Recognize valid reasoning without hiding an unsupported conclusion.**

[English](README.md) | [한국어](README.ko.md) | [Rust guide](docs/RUST_GUIDE.md) | [Textbook](book/Percent-Grading-Textbook.md)

Proposed by **Yubin Kim**. Experimental research and engineering project. Current implementation: **Rust 0.2.0**, with the Python 0.1.1 teaching prototype retained.

## Why this project began

An answer can differ from the reference while preserving a valid component, a sound principle, or a conclusion that holds under another condition. A single correct/incorrect label can hide those distinctions.

The original thought experiment asked whether only A deserves recognition when A is the expected answer. Another symbol might share a component with A without being A. The engineering question is:

> What is supported in this answer, what fails, under which conditions, and what remains unresolved?

Human grading already includes partial credit and analytic rubrics. Percent Grading investigates whether explicit language contracts and AI can make broader inspection more consistent, transparent, and useful.

## What the Rust program does

1. Compile a grading contract through the actual [RelSpec/AHAA compiler](https://github.com/kimyubin33/relspec-lang).
2. Validate a strict grading schema and freeze the contract with a SHA-256 identity.
3. Ask **OpenAI and Claude only** to choose predefined anchors and cite exact answer excerpts.
4. Reject invented criteria, anchors, numerical score fields, or unsupported quotations.
5. Accept a criterion only when the two validated judgments agree on its anchor.
6. Calculate fixed points, preserve disagreements, and save a replayable audit record.

The models classify evidence. **Rust controls the permitted points and the arithmetic.** This does not force the models to interpret language identically: disagreement produces review, rather than a hidden average. A saved run can be replayed without making another model call.

## The apple example

“What color is an apple?” is underspecified. The example contract explicitly describes a red-skinned specimen under broad white illumination and asks for its color, an explanation, and relevant conditions.

| Answer feature | How the profile handles it |
|---|---|
| Gives the expected red conclusion | Intent credit |
| Explains relevant reflection and absorption | Principle credit |
| Connects that mechanism to this specimen | Reasoning credit |
| States assumptions or distinguishes a changed scenario | Conditions credit |
| Proposes a testable future possibility | Separate future-exploration credit |

“Blue, because light is reflected” can contain a valid general principle while failing to justify blue in the specified setting. Blue illumination alone does not guarantee that a red surface looks blue; it can look dark. A defensible alternative needs explicit surface, illumination, or imaging conditions.

A short “red” answer satisfies the color-identification part. It does not demonstrate an explanation the expanded example task explicitly requests. The rubric must be stated before answers are graded; an ordinary color-only question should not acquire hidden reasoning requirements afterward.

## What the percentages mean

The current-task score measures fulfillment of the declared intent, principle, reasoning, and conditions criteria. The future score measures the quality of explicitly stated hypotheses or proposed alternatives. **Future credit does not increase current-task correctness.**

These are rubric scores, not probabilities of truth or forecasts of future success. Disagreement and unresolved criteria produce bounded reports. Their bounds describe the scoring policy, not statistical confidence intervals or protection against errors shared by both models.

Perspectives inform fixed criteria. Adding many overlapping theories does not multiply the available credit. The program supports declared perspectives; automatic discovery of thousands of theories remains future work.

## Quick start without API keys

Install a current stable Rust toolchain, clone this repository, and run from its root:

```text
cargo build --locked
cargo run --locked -- compile policies/apple.aha --out local-runs/apple.lock.json
cargo run --locked -- demo local-runs/apple.lock.json examples/rust/apple-blue.fixture.json
cargo test --locked
```

The compile command creates a new lock file and refuses to overwrite an existing one. Reuse the existing lock, or choose a new output path after changing a policy.

The demo uses **manually authored fixture judgments**, not real AI output. Red, future-scenario, and disagreement fixtures are also included. See the [Rust guide](docs/RUST_GUIDE.md) for their commands and the output interpretation.

## Live OpenAI and Claude evaluation

Configure `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` in your environment, then provide model IDs supported by your accounts:

```text
cargo run --locked -- grade local-runs/apple.lock.json examples/rust/apple-blue.txt --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL
```

A live grade calls both providers and may incur API charges. A ChatGPT subscription does not supply API access. No third provider or arbitrary API endpoint is supported. Request attempts, timeouts, output tokens, and response sizes are bounded; these limits are not a fixed monetary budget.

The guide explains local records, replay, offline judgment evaluation, and failure handling. Credentials are never part of the contract or saved report.

## Current scope

| Capability | Status |
|---|---|
| Rust CLI and real RelSpec/AHAA compilation | Implemented |
| Strict contract, fixed anchors, and content identity | Implemented |
| OpenAI Responses and Claude Messages adapters | Implemented |
| Two-judge reconciliation and exact quotation checks | Implemented |
| Separate present and future profiles | Implemented |
| Saved run replay and offline fixtures | Implemented |
| Large-scale theory retrieval and selection | Planned |
| Empirical advantage over existing grading | Not established |

The adapters require compatible model access and credentials for live use. Offline tests establish specified software behavior; they do not establish model accuracy or educational validity.

## Read and contribute

- [Rust guide: contract, CLI, API setup, and audit records](docs/RUST_GUIDE.md)
- [English textbook, including the Rust chapter](book/Percent-Grading-Textbook.md)
- [Architecture and trust boundaries](docs/ARCHITECTURE.md)
- [Experimental validation](docs/EXPERIMENTS.md)
- [Related work](docs/RELATED_WORK.md)
- [Roadmap](docs/ROADMAP.md)
- [Contributing](CONTRIBUTING.md)

The Python files and their [scoring](docs/SCORING.md) and [protocol](docs/PROTOCOL.md) documents describe the retained 0.1.1 prototype. They use a different input format; Rust does not silently accept Python judgment files.

## Intended benefits and research principles

The intended benefits are better recognition of valid alternatives, more specific feedback, visible disagreement, and decisions that can be inspected and replayed. These are hypotheses to test, not measured performance claims.

Keep task assumptions explicit. Credit only what an answer demonstrates. Preserve failed conclusions beside valid components. Version rubric changes and re-evaluate affected answers consistently. Publish counterexamples and unfavorable results.

## License and attribution

Original project code and documentation use the [MIT License](LICENSE). Vendored RelSpec/AHAA source is attributed separately in [UPSTREAM.md](vendor/relspec/UPSTREAM.md); the upstream snapshot did not include a license, so the root license does not relicense it.

Concept and project initiative: Yubin Kim. Documentation and implementation were developed with AI assistance. The project does not claim to have invented partial credit or rubric-based assessment.
