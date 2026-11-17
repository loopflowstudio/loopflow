# LOO-924 implementation notes

- 2026-11-17: Resume classifies binding with `bound_at` and primary scope,
  rather than restoring the old `Declared | Bound` WorkSource test: a direct
  `--task` launch records Declared but is Bare. This keeps the plan's attribution
  rule while restoring `SqliteStore::is_bare`.

- 2026-11-17: Keep WorkSource as attribution provenance: context and primary
  selection still read its variants. Launch lifetime no longer reads it.
- `codex_service_tier` and `chrome` stay on AgentConfig: they are provider
  launch options, independent of the selected model.

- 2026-11-17: Moving the document fixture to managed resume exposed a lost
  `--docs` selection on the fresh conversation's exec. Forward invocation
  settings in the existing conversation argv so the initial prompt keeps it.

- 2026-11-17, concept review: Discord Wave answers (`run::answer_bound`) stay
  `LaunchKind::Worker`. The plan's rule makes in-process callers batches,
  and the Discord bridge's own command is `lf --batch --wave`, which the
  table calls Bare. Kept Worker because that turn reported to the server on
  main and nothing shows dropping it is safe. Assumption, reversible: one
  line in `run.rs`.

- 2026-11-17: The transport row for a headless-recorded Session does not prove
  it can reopen interactively: `ops::human_session::run_refusal` rejects that
  Session before delegation. `docs/sessions.md` documents the restriction.
  Keep it in this review slice; LOO-958 owns Session command changes. The
  older plan's live batch → interactive resume check is therefore unavailable,
  not an acceptance proved by the launch-mode unit test.

- 2026-11-17, concept review of `55416284eb`: chose four launch-kind variants named
  for the cells (`Chat`, `Batch`, `WorkSession`, `Worker`) over renaming only
  `Run::Managed`. The plan allowed either. Reversible: a rename.
- Same review: with managed defined by turn reports alone, the Discord Wave
  answer staying a worker needs no exception. Whether it should report turns
  at all is still the assumption recorded above.
