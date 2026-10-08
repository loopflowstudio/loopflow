# Implementation choices — LOO-412, 2026-10-08

Jack Heart's custom-ref steers supersede host callbacks. The
[current design](work-on-another-machine-name.md) records the accepted boundary,
deletion cut and remaining acceptance. Earlier callback assumptions are retained
at `b040c7c8d:scratch/questions.md`.

- LOO-406's committed `4f9a8ea17` unifies saved-planning admission, but still has
  no causal peer import/export transaction. Its implementation stays separate;
  neither a second planner nor callback semantics fills that dependency.
- Planning remote selection must be explicit before ordinary commands can publish;
  no real planning data goes to the public code remote. Current transport tests
  use synthetic documents and disposable local remotes.
- Remove retained copied-planning adoption and issue-derived identity together
  with their replacement consumers after the common writer is available.

Retained pre-reconciliation check: `uv run --no-sync python scripts/test_network.py target/debug/deps/task_remote_tests-a43e8c3875a7fb96 --exact machine_selector_runs_a_skill_in_the_adopted_checkout_and_reuses_it --nocapture` — pass after merging main; proves adoption only.
