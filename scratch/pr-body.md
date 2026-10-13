Contributors currently update tests for harmless Wave guidance and Session terminology edits, and the Python suite repeats CI’s repository architecture scan. This change removes those maintenance obligations while keeping behavioral and generated-document checks.

## What changes

- Keep the dedicated repository architecture check and all 20 checker fixture cases.
- Delete two metric-prose tests and obsolete Wave/roadmap fragments; retain retired-ingest absence, catalog, parsing, command-resolution and safety-contract tests.
- Keep portable HTML freshness, absolute links and stylesheet portability; remove its exact title and Session-history fragments.
- Document each deleted assertion and retained proof in `docs/reviews/ci-test-coverage.md`.

Test code is net 49 lines smaller, with 21 assertions removed and three fewer cases. Prompt goldens and migration, recovery, Task ownership, network, account and process-safety proofs are unchanged. This implements the reviewed cleanup from [CI behavior and failure audit · PR #1543](https://github.com/loopflowstudio/loopflow/pull/1543). No elapsed-time saving or five-minute CI acceptance is claimed; CLI/disk preparation and macOS build overlap remain separate work.

## Checks

Passed: isolated architecture fixtures (20), portable HTML (1), Rust builtins (11), repository architecture map, cargo fmt, cargo clippy --all-targets -- -D warnings, and Ruff.

The changed-aware gate stopped at its resource-measurement preflight timeout (60 seconds), before product suites ran. Broader affected suites and the full matrix are deferred to hosted CI; no local full-suite pass is claimed.
