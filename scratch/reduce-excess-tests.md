# Reduce duplicate and brittle test maintenance

2026-10-13 · Task lf-6d3ff35 · implementation choices selected under Jack Heart’s shipping direction.

## Problem and outcome

Contributors maintain duplicate repository scans and prose assertions that fail on harmless editorial changes. Jack Heart authorized a small reduction with all useful regression protection retained. This is infrastructure-only; the observable win is fewer assertions to update after editing Wave guidance or Session terminology, while architecture drift and stale portable HTML still fail CI. No elapsed-time improvement or five-minute CI claim is selected.

## Chosen approach and findings

This slice removes `test_current_repository_has_no_unexplained_architecture` from Python: dedicated CI `architecture-check` runs the same `check_repository` implementation and remains the real-repository owner. All 20 fixture cases remain.

In `builtins.rs`, remove `wave_goal_authoring_keeps_live_metrics_wave_owned`, `project_guidance_sponsors_metrics_and_task_workers_propose_them`, and the seven prose assertions plus embedded waves document in `wave_model_is_embedded_in_prompts_and_docs`. Retain its ingest-absence assertion as `retired_ingest_skill_is_not_registered`. Catalog, parse, resolution and explicit safety-contract tests remain unchanged. These fragments cannot prove agent metric judgment or planning outcomes; skill/document review owns their meaning.

Remove title and exact Session-history wording from the portable HTML test. Its existing renderer `--check` is the only freshness assertion owner found in the test suites; retain it and the absolute execution link and absent relative stylesheet checks. No duplicate freshness test exists to remove. Missing/stale artifacts still fail the renderer. Source docs and renderer remain authoritative; committed HTML is their derived delivery artifact.

The existing audit (`docs/reviews/ci-test-coverage.md`, `ci-failure-history.md`) supplies historical evidence; no new CI-history research is needed. Direct assertion inspection confirms this narrow scope. Blanket prompt-golden deletion is rejected: content selection, ordering, omission and escaping have not been established as redundant. Replacing strings with new synonyms would retain the maintenance burden without proving behavior.

## Boundaries and remaining work

No production APIs, fixtures, migration/data, recovery, cross-Task ownership, network isolation, account isolation or process-safety tests change. CLI/disk preparation lf-8f870b1 and macOS compile overlap lf-ddf2d53 remain separate. Success keeps useful failure signals; failure would remove a distinct safety or delivery boundary. The retained-proof mapping belongs in the existing audit document before scratch cleanup.

Focused checks: isolated Python architecture fixtures, isolated portable website test, isolated Rust `builtins::tests`, and the repository architecture command. Gate: changed-aware `uv run python scripts/test.py` once, including affected Rust/Python/website suites and formatting/clippy. Expected: retained checks pass and no production diff. Hosted CI owns the full matrix, including Linux installation and macOS process safety. Delivery: supported Loopflow commit/land, authoritative merge reconciliation, then Task completion only after accepted proof.

Done when the test diff is net smaller, the durable assertion map is present, one repository architecture owner and one portable freshness owner remain, affected checks and full CI pass, and merge is verified. Remaining: hosted full CI, publication and verified delivery. Test code is net 49 lines smaller (21 assertions removed; three fewer test cases).

Check: isolated pytest architecture fixtures (20), portable HTML (1), Rust builtins (11), repository architecture, cargo fmt, clippy --all-targets -- -D warnings and Ruff passed; scripts/test.py resource preflight timed out at 60s before product suites, broader affected/full matrix deferred to hosted CI without retry.
