# Publication setup

Repository name: percent-grading

Description: AI-assisted multidimensional grading with evidence, partial validity, and explicit uncertainty.

Suggested topics: ai, assessment, grading, rubric, llm-as-a-judge, education, research.

Place README.md at the repository root. README.ko.md provides the Korean introduction; the program, technical documents, examples and textbook use English.

Keep .github/workflows/tests.yml so hosted tests run after publication. Do not include credentials, local raw runs, caches or private learner data.

Version 0.2.0 passed 48 Rust tests on Windows with Rust 1.99.1, plus formatting and Clippy checks, on 2026-11-12. The retained Python prototype passed its 16 tests on Python 3.14.4. The included GitHub Actions workflow checks Rust on Windows and Ubuntu and Python on three versions; see the repository's Actions tab for hosted results. These tests verify specified software behavior, not the validity of AI grading judgments. No paid live-provider evaluation was performed for this release.
