# LOO-449 — checkout-owned Wave documents

Jack Heart selected direct repository-file ownership on 2026-10-09 (steer
507b173a-3969-469d-8c91-028d57ae225f). This is the implementation plan for that
accepted direction, not authorization to publish or land.

## Design

Read `wave/<address>/` Markdown from the selected checkout on every context
assembly. Read GOAL.md frontmatter and summaries there too; absence is absence,
never a stored fallback. Keep ancestor ordering and additional Markdown context.
Registry identity, Projects, execution and provider links stay in SQLite.
Explicit creation creates missing GOAL.md/MEMORY.md without overwriting authored
files. Canonical repository identity must not redirect authoring into main.

## Delete — do not maintain

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
- `ops/pm.rs`: legacy Team validation, PM test override; sync summary already
  uses config reader. Frontmatter writers retain the objective body.
- `store/sqlite/wave_documents.rs`: document CRUD; ensure/import (also workflows).
- `store/sqlite.rs`: Wave registration import and retirement comparison.
- `store/migrations.rs`: import existing registered Waves during draft application.
- CLI edit dispatch is the only whole-document mutation API.

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

## Reconciled implementation

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

Release child memory's operation-entry lesson informed native launch coverage;
its remaining publication/schedule evidence is unrelated and unchanged. No other
immediate Infrastructure child memory exists in this checkout.

## Remaining

Gate owns affected planning/PM/cron/metrics/context suites, Linear sync entry-path
coverage and materialized migration checks. CI owns the full platform matrix.
Reconcile LOO-444's context-reader overlap when integrating its committed change;
no transport changes or installed-store access occurred here. `wave_workflows`
removal remains Jack's open decision, not a dependency for this cut.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and focused `cargo test -p loopflow` filters (Wave files/draft upgrade/relocation, config, checkout context/list/status and consecutive native launches) pass; gate/CI own the broader suites.

