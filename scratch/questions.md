# Implementation choices — LOO-412, 2026-10-08

Jack Heart's custom-ref steers supersede host callbacks. The
[current design](work-on-another-machine-name.md) records the accepted boundary,
deletion cut and remaining acceptance. Earlier callback assumptions are retained
at `b040c7c8d:scratch/questions.md`.

- LOO-406's common writer is now integrated from published `e68f2a423`.
  LOO-412 must add peer import/export and ordering here; those are not prerequisites
  for the parent. Do not wait for its remaining Linear delivery work.
- Follow Jack Heart's newer policy: Linear wins observed conflicts; otherwise host
  preference where appropriate, then last-write-wins with retained losing edits.
  Use user-keyed destination selection by default, explicit opt-in shared planning.
  Exact stable key setup and deterministic ties are implementation choices here.
- Planning remote selection must be explicit before ordinary commands can publish;
  no real planning data goes to the public code remote. Current transport tests
  use synthetic documents and disposable local remotes.
- Remove retained copied-planning adoption and issue-derived identity together
  with their replacement consumers on the integrated common writer.

Retained pre-reconciliation check: `uv run --no-sync python scripts/test_network.py target/debug/deps/task_remote_tests-a43e8c3875a7fb96 --exact machine_selector_runs_a_skill_in_the_adopted_checkout_and_reuses_it --nocapture` — pass after merging main; proves adoption only.
