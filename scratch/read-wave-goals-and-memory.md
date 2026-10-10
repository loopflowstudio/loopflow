# LOO-449 — checkout-owned Wave documents

Jack Heart selected direct repository-file ownership on 2026-10-09 (steer
507b173a-3969-469d-8c91-028d57ae225f). This is the implementation plan for that
accepted direction. Jack Heart’s October 9 steer `2991ceb1-c0a7-40a7-90db-b61280abf36d`
authorizes publication after a mocked sync proves direct file-edit propagation;
it does not authorize landing or settle divergent-checkout selection.

## Design

Read `wave/<address>/` Markdown from the selected checkout on every context
assembly. Read GOAL.md frontmatter and summaries there too; absence is absence,
never a stored fallback. Keep ancestor ordering and additional Markdown context.
Registry identity, Projects, execution and provider links stay in SQLite.
Explicit creation creates missing GOAL.md/MEMORY.md without overwriting authored
files. Canonical repository identity must not redirect authoring into main.

## Delete — do not maintain

The document-storage cut below is complete, including removal of the unused
`update_wave_agent_config` writer, its field-edit helpers and exclusive tests;
agent policy is authored in GOAL.md. PM metadata updates remain.

- `wave_documents` table and revision triggers in `local_planning.sql`.
- SQLite document getters/setter and document-import hooks/tests, including
  stored-document retirement comparisons. Preserve workflow import independently.
- `WaveCommand::Edit`, dispatch, flags and reference documentation.
- Stored-document prompt/config readers and PM test-only alternate reader.
- Fixtures asserting file edits are ignored or documents survive file removal.

## Reader/writer inventory

- `engine/prompt.rs::gather_wave_docs`: launch/context, all ancestor Markdown.
- `work/wave/config.rs`: config, summary, chat and frontmatter readers; shared
  by launch budgets/agents, cron, metrics, PM and other Wave surfaces.
- `lf/commands/waves.rs::snapshot_wave`: list/status/roadmap summary.
- `ops/pm.rs`: legacy Team validation and sync summary use the config reader. Frontmatter writers retain the objective body.
- `store/sqlite/wave_definitions.rs`: registration, missing-file creation and
  retained Workflow import; document CRUD/import is deleted.
- `store/sqlite.rs`: registration imports and retirement compares only Workflows.
- `store/migrations.rs`: import registered Workflows during draft application.
- Whole-document CLI edit dispatch is deleted.

## wave_workflows — retained, decision open

`ops/project.rs` reads it for Project-scoped catalog, source, selection and
workflow loading (including Task workflow capture). `select_project_workflow`
writes definition and Project content transactionally. Provisioning and draft
migration import `.lf/workflows/*.{yaml,yml}` once; retirement compares saved
workflow definitions. These remain unchanged apart from separating their import
from the removed document import. Jack has not selected their removal.

## Preservation and acceptance

File edits must affect the next context/list/status/sync read; two checkouts may
have different goals without sharing text. Missing files cannot resurrect old
text. Ensure is idempotent and preserves authored bytes, IDs and workflow import.
Registration/relocation preserve execution and never rewrite authored documents;
addresses select matching checkout directories, which remain ordinary files.
Exercise draft upgrade with no document table and retained workflows; reject
`wave edit`; cover creation and direct edit propagation. Gate owns affected suites,
CLI launch and Linear sync acceptance; CI owns the full materialized matrix.

## Reconciled implementation (2026-10-09)

The deletion cut is implemented. Context reads checkout Markdown on every assembly;
config and summaries read GOAL.md directly. Creation writes missing files in the
invoking checkout while retaining canonical registry identity. List/status/roadmap
share checkout resolution with PM validation. PM metadata edits require an existing
goal and preserve its body. Skills and docs teach direct file edits.

Review found two canonicalization traps: Task creation could seed main instead of
its worktree, and status/PM validation could read main after resolving a shared
Wave identity. Both now separate the checkout's bytes from canonical DB lookup.
Config fixtures no longer need an artificial stored-document owner. Native launch
fixtures still register Wave identity; deleting document storage does not remove
launch attribution. Relocation keeps authored files unchanged (assumption above).

Compression shares the goal-config reader with registration, removes the generic
stored-document reader and file importer, and keeps config fixtures filesystem-only.
Workflow import still prefers `.yaml` and preserves saved definitions; help now
states that ownership rather than claiming checkout reads.

Release is the only immediate child with memory in this checkout. Its goal,
memory headings and relevant release/recovery sections were inspected; the full
historical file was not reread. Its operation-entry lesson applies to Linear sync
as well as native launch: a config-reader proof alone cannot prove dispatch uses
the invoking checkout. Release-specific evidence remains in the child.

## Sync counterexample (2026-10-09)

The previous plan incorrectly described Linear sync as a summary reader. At
`c756414c8`, public dispatch in `lf/commands/ops/mod.rs::sync_planning` calls
`ops/pm.rs::pm_sync` / `pm_sync_async`. That operation reads checkout PM bindings,
validates ownership, optionally renames Initiatives, adopts legacy Projects and
refreshes snapshots. It never reads or sends the Wave summary. Its only Initiative
update is `LinearClient::rename_wave`; `UPDATE_INITIATIVE_MUTATION` accepts only
ID and name. `pm_init_async` reads the checkout summary, but sends it only when
creating an Initiative, not when reconnecting an existing binding.

Thus the requested test cannot pass through the current sync entry point. This
is missing behavior, not merely missing verification. No new outbound writer or
checkout-selection policy was added under the narrow test-and-publish steer.
Publication remains withheld; the acceptance has not been weakened to testing
creation or the config reader. A reviewed sync-writing design is needed before
continuing this dependent slice. Jack’s choice between branch and main remains
open; currently sync publishes neither checkout’s summary.

## Remaining

Resolve the summary-writing mismatch above, then prove two sync invocations around
a direct GOAL.md edit against a mocked provider, without import/save. Preserve
separate main/worktree bytes and state the observed selection in PR notes without
presenting it as Jack’s policy choice. Gate owns broader affected suites and
materialized migrations; CI owns the platform matrix. Reconcile LOO-444’s context
reader at integration; no integrated result is claimed. `wave_workflows` stays
unchanged, with its consumers inventoried above.

Checks: source trace confirms sync has no summary write; no code changed or tests rerun in this reconciliation. Prior `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and network-isolated `cargo test --offline -p loopflow --lib` with `work::wave::config::tests` / `store::sqlite::wave_definitions::tests` pass (11 tests); gate/CI own broader acceptance. Earlier checkout/native-launch proofs remain at `3db1e0c00:scratch/read-wave-goals-and-memory.md`.
