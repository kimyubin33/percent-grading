# Percent Grading

**AI-assisted grading that preserves partial validity, evidence, and uncertainty.**

[English](README.md) | [한국어](README.ko.md) | [Textbook](book/Percent-Grading-Textbook.md)

Proposed by **Yubin Kim**. Experimental research and engineering project.

## Why this project began

An answer can fail to match the reference while still containing a valid component, a useful relationship, or a conclusion that holds under a particular condition. A single correct/incorrect label can hide those distinctions.

The original thought experiment was simple: if A is the required answer, must only A receive recognition? In an idealized decomposition, another symbol might share one of A's strokes. That overlap deserves description, even though it does not establish that the other symbol is A.

Percent Grading turns that intuition into an engineering question:

> What is supported in this answer, what fails, under which conditions, and what remains unresolved?

Human grading already includes partial credit and analytic rubrics. This project explores whether AI can help apply more relevant perspectives, preserve their evidence, and make disagreements easier to inspect at scale.

## What the percentage means

A percentage represents **fulfillment of an explicitly defined rubric**. It is not a universal percentage of truth, a probability of correctness, or a measure of a person's intelligence.

We separate four useful dimensions:

| Dimension | What it checks |
|---|---|
| Components | Required entities and properties |
| Relationships | Logical, causal, spatial, or temporal connections |
| Conditions | Assumptions, scope, and exceptions |
| Goal fulfillment | Whether the response accomplishes the task |

A correct component does not automatically establish a correct relationship or a successful answer. Critical requirements preserve that distinction.

## Intended benefits

| Benefit to investigate | How the design supports it | What must be verified |
|---|---|---|
| More useful feedback | Criterion-level evidence and repair guidance | Whether learners can improve more effectively |
| Recognition of alternative answers | Meaning-based judgment with accepted alternatives | Paraphrase acceptance without rewarding vague answers |
| More reviewable decisions | Quotations, reasons, rubric versions, and explicit abstention | Whether the evidence actually supports the judgment |
| Broader inspection | A planned registry of theories and evaluation lenses | Whether additional perspectives improve quality at an acceptable cost |
| Visible uncertainty | Coverage and completion bounds beside the score | Whether users interpret the outputs correctly |

These are intended advantages, not measured performance claims.

## How it works

1. Define the task, assumptions, reference, and acceptable alternatives.
2. Fix criteria, weights, score anchors, and critical requirements.
3. Generate a grading request for a chosen AI evaluator.
4. Obtain one reviewed judgment per criterion, with exact answer excerpts.
5. Validate the input and calculate scores deterministically.
6. Read the criterion profile, critical failures, and coverage before the total.

In the larger architecture, multiple perspectives can inform each criterion. They do not each receive their own share of the grade merely because they exist. Repeating the same idea under thousands of names must not create thousands of independent votes.

“All perspectives” is an open-ended ambition. A finite system must report which perspectives it examined, why they were relevant, and where coverage remains limited.

## Current capabilities

| Capability | Status |
|---|---|
| Evaluation-request generation | Implemented |
| JSON contract and exact quotation checks | Implemented |
| Fixed-rubric aggregation and critical gates | Implemented |
| Coverage and unresolved-score bounds | Implemented |
| Examples and unit tests | Included |
| English textbook and bilingual introduction | Included |
| Automatic AI API calls | Planned |
| Theory retrieval and multi-judge disagreement resolution | Planned |
| Empirical advantage over human or conventional grading | Not established |

The bundled judgments are **manual teaching fixtures**, not AI benchmark results.

## Quick start

Python 3.10 or later. The core uses the standard library only.

~~~text
python -m percent_grading prompt examples/osmosis.case.json
python -m percent_grading aggregate examples/osmosis.case.json examples/osmosis.judgments.json
python -m unittest discover -s tests -v
~~~

Copy the first command's output into your chosen AI service. Save only the returned JSON object as a UTF-8 file, then supply that path to the aggregate command. No API request is made by the current CLI.

The osmosis fixture produces approximately **83.33% observed fulfillment at 90% coverage**, with completion bounds of **75–85%**. These are arithmetic results from illustrative manual judgments, not validated measurements of learning.

The bounds account for unresolved criteria while accepted scores remain fixed. They are not statistical confidence intervals and do not include AI judgment errors.

## Read and learn

- [Textbook: foundations, examples, laboratory, exercises, and solutions](book/Percent-Grading-Textbook.md)
- [Concept and origin](docs/CONCEPT.md)
- [Scoring rules](docs/SCORING.md)
- [Architecture and perspective selection](docs/ARCHITECTURE.md)
- [Input contract and AI workflow](docs/PROTOCOL.md)
- [Experimental validation](docs/EXPERIMENTS.md)
- [Related work](docs/RELATED_WORK.md)
- [Roadmap](docs/ROADMAP.md)
- [Contributing](CONTRIBUTING.md)

## Research principles

Keep the task and criteria explicit. Preserve valid fragments without hiding failed goals. Distinguish missing requirements from unresolved judgment. Record disagreement rather than hiding it in an average. Evaluate new perspectives against their cost. Publish counterexamples and unfavorable results.

This prototype is intended for research and formative evaluation. Using it for consequential grades requires domain validation and a meaningful review process.

## License and attribution

Code and documentation are released under the [MIT License](LICENSE).

Concept and project initiative: Yubin Kim. Documentation and initial implementation were developed with AI assistance. The project does not claim that partial credit or rubric-based assessment originated here.
