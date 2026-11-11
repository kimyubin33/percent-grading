# Publication setup

Repository name: percent-grading

Description: AI-assisted multidimensional grading with evidence, partial validity, and explicit uncertainty.

Suggested topics: ai, assessment, grading, rubric, llm-as-a-judge, education, research.

Place README.md at the repository root. README.ko.md provides the Korean introduction; the program, technical documents, examples and textbook use English.

Keep .github/workflows/tests.yml so hosted tests run after publication. Do not include credentials, local raw runs, caches or private learner data.

The English 0.1.1 package passed all 16 local tests on Python 3.14.4 on 2026-11-11. The included GitHub Actions workflow runs the checks on pushes and pull requests; see the repository's Actions tab for hosted results. These tests verify the implementation's specified behavior, not the validity of AI grading judgments.
