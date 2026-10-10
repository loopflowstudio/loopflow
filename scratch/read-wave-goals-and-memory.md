# LOO-449 — checkout-owned Wave documents

Jack Heart selected checkout ownership on October 9, 2026
(`507b173a-3969-469d-8c91-028d57ae225f`). His later steer
`2991ceb1-c0a7-40a7-90db-b61280abf36d` authorizes publication after a mocked
sync proves direct GOAL.md edit propagation, not landing. His October 10 steer
`a7f4cbaf-0d87-47d2-83b8-940e5ce7b461` resolves the policy: Linear sees only the
merged default-branch goal; launch context, list, status and roadmap remain
checkout-based. His October 10 approval of PR #1521
(`05b04fa8-9fa7-4c3f-be41-300b8f68dacd`) authorizes landing with that fix.

## Design and preservation

Read `wave/<address>/` Markdown from the selected checkout on every assembly,
including ancestors and additional Markdown. Read GOAL.md config and summaries
there too, except Linear sync reads the committed default-branch goal. Missing
files mean absence, never a stored fallback. SQLite retains
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
Workflow import and PM metadata updates remain separate. The remaining PM
frontmatter writer keeps its read/parse/update/write sequence together; the three
single-caller helpers left by the deleted editing API are gone. The sync fixture
uses one commit helper for default and feature branches.

## Surviving readers and writers

- `prompt/mod.rs::gather_wave_docs`: fresh checkout/ancestor Markdown.
- `work/wave/config.rs`: checkout config, summary, chat and PM frontmatter;
  shared by launch policy, cron, metrics and PM.
- `lf/commands/waves.rs`: list/status/roadmap summaries and PM validation use
  the invoking checkout for its repository, otherwise the recorded repository.
- `ops/pm.rs`: checkout bindings and legacy Team validation; default-branch summary sync.
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

Sync resolves the existing default-branch name from origin/HEAD (main when absent),
pins its local refs/heads commit once, and reads goal blobs from that commit.
It neither fetches nor checks out another branch. Dirty default-checkout files and
committed feature-branch edits cannot supply the summary. Missing refs fail;
missing GOAL.md blocks outbound mutation. There is no checkout fallback.
Binding and other planning behavior is unchanged.
Preflight carries the checked Initiative and changed fields directly into apply.
Only changed name/description fields are sent through the existing update operation; standalone rename omits description. Provider errors or missing
success acknowledgements return failure, without automatic write retry.

Missing GOAL.md is diagnosed before mutation; an existing empty objective can
clear the summary. Review removed an existence-check/read race: preflight parses
one successful file read instead of a reader whose missing-file result is empty.
No schema, Workflow, membership or execution owner was added.

The stateful mocked-provider test uses the public `sync_planning` dispatcher
from a linked feature checkout, with a non-main default branch. Committed branch
edits and dirty default-checkout edits leave the provider unchanged; committing the
same text on the default branch sends it without import/save. It also covers no-op,
plan-only, missing goal, rejected update without retry and intentional empty text.
The existing foreign-Project and legacy Workflow fixtures now commit their goals.

## Remaining and PR notes

Branch/main selection is resolved by Jack's October 10 decision. Jack approved
PR #1521 and authorized landing with the published default-branch fix. The mocked
sync dispatch proves provider behavior; installed acceptance remains unproved.
Review retained one pinned Git revision rather than reading another checkout's
mutable files, preventing dirty default-checkout text from leaking to Linear.

Gate exercised the materialized migration graph, public checkout/status/context
fixtures and composed stand-in launch paths; CI owns the platform matrix. Main `be4a2b2af` is integrated; its module move
places the context reader in `prompt/mod.rs` and Git helpers in `git/mod.rs`.
The retained revision-file reader and updated calls pass the focused sync proof
(recorded at `bfba1d78f`). Configured providers and installed
acceptance remain unproved. Creation/relocation must retain
IDs, execution and Workflow import; deletion must not resurrect stored text.
Earlier checkout/native-launch proofs: `3db1e0c00:scratch/read-wave-goals-and-memory.md`.
Canonicalization fixes and Release's operation-entry lesson:
`45c184227:scratch/read-wave-goals-and-memory.md`; a config-reader proof alone
cannot establish public sync behavior.

Gate found three stale fixture assumptions: relocation implicitly carrying stored
documents, budget fixtures provisioning unrelated goals, and goldens retaining old
guidance/provisioning. Fixtures now move authored directories explicitly, read
files without registration, and expect the direct-edit instructions. Production
ownership needed no further change.

Release is the only immediate child directory with memory. Its top-level goal
and full memory were read; the operation-entry lesson above remains applicable,
with release-specific history retained there.

Checks: `uv run python scripts/test.py --reuse-passing` passed architecture/website/fmt/Clippy and 2,388 materialized Rust tests, with 18 skipped and three stale-fixture failures; after repair, fmt/Clippy and `scripts/materialize_rust_tests.py -- cargo … nextest run --all -E 'test(release_task_prompt_follows_parent_rename_and_reparenting) | test(repository_ancestor_and_selected_wave_memory_share_one_budget) | test(golden_prompts_match)'` passed both unit repairs, and its golden-only rerun passed after removing provisioning; no second broad pass; CI owns remaining platform/skip coverage, installed acceptance unproved.
