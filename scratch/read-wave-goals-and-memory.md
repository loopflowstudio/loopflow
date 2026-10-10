# LOO-449 — checkout-owned Wave documents

Jack Heart selected checkout ownership on October 9, 2026
(`507b173a-3969-469d-8c91-028d57ae225f`). His later steer
`2991ceb1-c0a7-40a7-90db-b61280abf36d` authorizes publication after a mocked
sync proves direct GOAL.md edit propagation, not landing or a branch/main policy.

## Design and preservation

Read `wave/<address>/` Markdown from the selected checkout on every assembly,
including ancestors and additional Markdown. Read GOAL.md config and summaries
there too. Missing files mean absence, never a stored fallback. SQLite retains
identity, Projects, execution, provider links and Workflow definitions.
Creation seeds missing GOAL.md/MEMORY.md without overwriting authored bytes.
Canonical repository identity must not redirect checkout reads or writes into main.
Registry relocation leaves authored files in place; direct authoring owns their move.
PM frontmatter updates preserve the objective body and require an existing goal.

## Delete — do not maintain

Complete: `wave_documents` and its revision triggers in the unreleased
`local_planning.sql` draft; document CRUD/import/fallback readers;
`WaveCommand::Edit` and flags; unused `update_wave_agent_config` and helpers;
their exclusive stored-copy fixtures. No compatibility path or new migration.
Workflow import and PM metadata updates remain separate.

## Surviving readers and writers

- `engine/prompt.rs::gather_wave_docs`: fresh checkout/ancestor Markdown.
- `work/wave/config.rs`: checkout config, summary, chat and PM frontmatter;
  shared by launch policy, cron, metrics and PM.
- `lf/commands/waves.rs`: list/status/roadmap summaries and PM validation use
  the invoking checkout for its repository, otherwise the recorded repository.
- `ops/pm.rs`: checkout bindings and legacy Team validation. Only Initiative
  creation sends the checkout summary; sync does not update it.
- `store/sqlite/wave_definitions.rs`: registration, missing-file creation and
  Workflow import. `store/sqlite.rs` and `store/migrations.rs` retain only
  Workflow import/retirement comparisons, not document storage.

`wave_workflows` stays unchanged. `ops/project.rs` consumes its Project catalog,
source, selection and workflow loading, including Task capture.
`select_project_workflow` saves definition and Project content transactionally.
Provisioning and draft migration import `.lf/workflows/*.{yaml,yml}` once,
prefer `.yaml` and preserve stored edits. Retirement compares saved definitions.
Jack has not selected its removal.

## Remaining: summary sync and publication

At `45c184227`, `lf/commands/ops/mod.rs::sync_planning` dispatches to
`ops/pm.rs::pm_sync_async`, which validates ownership, optionally renames
Initiatives, adopts legacy Projects and refreshes snapshots. It neither reads
nor sends summaries. `LinearClient::rename_wave` / `UPDATE_INITIATIVE_MUTATION`
accept only ID/name. `pm_init_async` sends the summary only on creation, not
reconnection. The requested mocked sync test therefore needs missing behavior,
not merely verification; testing creation or the config reader would not satisfy it.

Publication remains withheld pending a sync-writing design. Branch-versus-main
selection remains open for Jack; currently sync publishes neither summary.
After resolving the writer, prove two sync invocations around a direct GOAL.md
edit against a mocked provider, with no import/save. Preserve distinct main and
worktree bytes and report observed selection without claiming Jack chose it.

Gate owns broader affected suites, public dispatch and migration acceptance;
CI owns the platform matrix. Reconcile LOO-444's context reader at integration;
no integrated or installed acceptance is claimed. Creation/relocation must retain
IDs, execution and Workflow import; deletion must not resurrect stored text.
Earlier checkout/native-launch proofs: `3db1e0c00:scratch/read-wave-goals-and-memory.md`.
Canonicalization fixes and Release's operation-entry lesson:
`45c184227:scratch/read-wave-goals-and-memory.md`; a config-reader proof alone
cannot establish public sync behavior.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` pass; documentation/comment-only edits reuse the prior 11 focused config/registration passes; gate/CI own broader acceptance and the missing sync proof.
