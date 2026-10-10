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

### Narrow writer design (draft, October 9)

Jack's requested behavior requires an outbound summary update, not a different
reader test. The implementation remains outstanding; a new storage owner, import
step or broad planning rewrite is unnecessary.

- Keep `sync_planning`'s supplied checkout path through `pm_sync_async`. Read
  each selected GOAL.md summary during preflight, before any provider mutation.
  Preserve existing Team/Initiative ownership checks and `--plan`'s no-write rule.
- Retain the summary already returned by `LinearClient::list_waves` instead of
  projecting only ID/name. Compare the checkout summary using the same provider
  description normalization as creation; report and write only changed values.
- Extend the existing Initiative update operation to accept a description without
  making standalone rename erase it. Send only name/description, preserving
  membership, Projects, execution and `wave_workflows`. Surface provider errors;
  do not add automatic write retries or claim successful sync after failure.
- Missing GOAL.md must not become a destructive empty-summary write: report its
  absence before mutation. An existing, intentionally empty objective is distinct.
  This preservation choice is a draft assumption, not a new stored fallback.
- Prove two real sync invocations against a stateful mocked provider around a
  direct file edit. Assert the provider's resulting summary, with distinct main
  and checkout text; also cover unchanged sync, plan-only, missing file and
  failed update. Isolate credentials/store and exercise public checkout dispatch.

Current behavior: sync receives the caller's checkout for file bindings but sends
no summary; creation alone sends its checkout summary. The proposed writer uses
that existing path, without claiming Jack selected branch-over-main policy.
That policy stays explicitly open for Jack in PR notes. Publication remains
withheld until the mocked sync acceptance passes; this realign does not publish.

Gate owns broader affected suites, public dispatch and migration acceptance;
CI owns the platform matrix. Reconcile LOO-444's context reader at integration;
no integrated or installed acceptance is claimed. Creation/relocation must retain
IDs, execution and Workflow import; deletion must not resurrect stored text.
Earlier checkout/native-launch proofs: `3db1e0c00:scratch/read-wave-goals-and-memory.md`.
Canonicalization fixes and Release's operation-entry lesson:
`45c184227:scratch/read-wave-goals-and-memory.md`; a config-reader proof alone
cannot establish public sync behavior.

Release is the only immediate child directory with memory. Its top-level goal
and full memory were read; the operation-entry lesson above remains applicable,
with release-specific history retained there.

Checks: `git diff --check` passes; source trace confirms the absent sync writer and existing provider summary read; no code changed or suites rerun. Prior fmt/Clippy and 11 focused config/registration passes remain applicable; gate/CI own broader acceptance.
