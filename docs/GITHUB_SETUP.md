# Publication setup

Repository name: percent-grading

Description: AI-assisted multidimensional grading with evidence, partial validity, and explicit uncertainty.

Suggested topics: ai, assessment, grading, rubric, llm-as-a-judge, education, research.

Place README.md at the repository root. README.ko.md provides the Korean introduction; the program, technical documents, examples and textbook use English.

Keep .github/workflows/tests.yml so hosted tests run after publication. Do not include credentials, local raw runs, caches or private learner data.

Version 0.4.0 adds claim-level source audits and common evidence-revision plans. Its bundled evidence sources and provider-role reviews are explicitly synthetic manual fixtures. The release checks include offline policy validation, evidence aggregation, deterministic grading gates, audit resume/replay, and cohort revision behavior. See the Actions run attached to the published commit for the actual hosted results. Source quality and live-provider accuracy are not established by these tests, and no paid model calls are required by the fixture workflow.

Version 0.3.0 passed 80 Rust tests on Windows with Rust 1.99.1, plus formatting and Clippy checks, on 2026-11-12. The retained Python prototype has 16 tests. The included GitHub Actions workflow checks Rust on Windows and Ubuntu and Python on three versions; see the repository's Actions tab for hosted results. The public Crossref connectivity smoke check is non-blocking and must be inspected separately. These checks verify specified software behavior, not the validity of AI grading judgments. No paid live-provider evaluation was performed for this release.
