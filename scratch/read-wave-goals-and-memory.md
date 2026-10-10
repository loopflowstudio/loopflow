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
- `ops/pm.rs`: checkout bindings, legacy Team validation and summary sync.
- `store/sqlite/wave_definitions.rs`: registration, missing-file creation and
  Workflow import. `store/sqlite.rs` and `store/migrations.rs` retain only
  Workflow import/retirement comparisons, not document storage.

`wave_workflows` stays unchanged. `ops/project.rs` consumes its Project catalog,
source, selection and workflow loading, including Task capture.
`select_project_workflow` saves definition and Project content transactionally.
Provisioning and draft migration import `.lf/workflows/*.{yaml,yml}` once,
prefer `.yaml` and preserve stored edits. Retirement compares saved definitions.
Jack has not selected its removal.

## Summary sync implemented; publication remains separate

The October 9 draft is implemented. `sync_planning` preserves its supplied
checkout through `pm_sync_async`; preflight captures summaries before mutations,
using creation's description normalization. Initiative observations retain their
summaries. Only changed name/description fields are sent through the existing
update operation; standalone rename omits description. Provider errors or missing
success acknowledgements return failure, without automatic write retry.

Missing GOAL.md is diagnosed before mutation; an existing empty objective can
clear the summary. Review removed an existence-check/read race: preflight parses
one successful file read instead of a reader whose missing-file result is empty.
No schema, Workflow, membership or execution owner was added.

The stateful mocked-provider test calls the public `sync_planning` dispatcher
twice around a direct edit in a disposable linked checkout, whose main checkout
retains different text. Resulting provider summaries prove propagation without
import/save. It also covers unchanged sync, plan-only, missing file, rejected
update without retry, and intentional empty text. Existing foreign-Project and
legacy Workflow-conversion sync fixtures pass.

## Remaining and PR notes

Jack's mocked-sync publication condition is satisfied by that source proof.
Publication belongs to the caller's delivery step; no publication or landing
occurred here. Branch-versus-main policy remains explicitly open for Jack:
implementation preserves the caller's existing checkout selection, not a newly
approved default. CLI process-level dispatch and installed behavior remain gate
or CI evidence, distinct from the public command-function proof.

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

Checks: `cargo check -p loopflow --lib`, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and three focused sync tests pass; gate/CI own broader and installed acceptance.
