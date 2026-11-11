# RelSpec / AHAA source provenance

Repository: https://github.com/kimyubin33/relspec-lang

Pinned commit: `de1e956555dcfafc5b62324ea6ed3f62655c6ba3`

Package: `aha` 0.14.0; language version: 0.14.

The files `Cargo.toml`, `Cargo.lock`, and `src/*` are copied from that exact revision, without source changes. This directory is a path dependency so the grading contract is checked by the real upstream compiler and does not depend on a moving branch at build time.

The upstream tree at this revision contains no license file and the upstream Cargo manifest declares no license. Do not infer an MIT license for these vendored files from the license of the surrounding Percent Grading project. They remain attributed to their upstream author. The project owner explicitly requested integration of their own repository; third-party redistribution must account for the upstream licensing status.

Only the public `aha::compile` API and its checked scene IR are used by Percent Grading. The wrapper accepts a smaller grading-contract subset and does not claim that upstream free-text values are semantically verified.
