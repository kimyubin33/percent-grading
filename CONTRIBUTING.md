# Contributing

Contribute a task, assumptions, answer, rubric version, expected judgment, observed result, and evidence. Remove personal data and private assessment materials.

For a new perspective, explain its source or hypothesis status, scope, assumptions, relevant criteria, and overlap with existing perspectives. Do not add criteria just to increase a favored answer's score.

Version changes made after inspecting responses and re-evaluate affected examples consistently. Preserve counterexamples as regression fixtures.

For Rust changes run `cargo fmt --package percent-grading -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`. Run `python -m unittest discover -s tests -v` for the retained Python prototype. Report test limitations and distinguish software behavior from grading validity. Never commit credentials or local grading records.

Do not edit vendored AHAA source silently. Record the upstream revision and preserve its exact files; see [provenance](vendor/relspec/UPSTREAM.md). Review the [Rust contract guide](docs/RUST_GUIDE.md) before adding grading criteria or new anchors.
