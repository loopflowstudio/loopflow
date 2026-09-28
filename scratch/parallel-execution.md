# Execution integration — 2026-09-28

The managed worker has read `scratch/parallel-work.md` and retains only the
execution model, public connection, shared files, consumers and core docs.
Reserved H7/performance files are untouched by this worker.

## Build slot

No Cargo batch is running for the managed worker. The last focused batch,
`exec-native-transfer`, passed in 25.19 seconds; all-target Clippy had already
passed in `exec-native-clippy` (15.75 seconds). Both finished before the parallel
allocation. No further Cargo command is queued. The next command and ready
request will be recorded here before seeking a supervisor slot.

Source and documentation edits continue. No global formatting or checkpoint
will include contributors' active changes. Native transport proof and its
limitations are recorded in `scratch/cutover/exec-ownership.md`; public lifecycle
integration remains unfinished.

## Shared Project integration in progress

The performance slot is released. No Cargo command has started here. The main
worker is updating ProjectPlan constructors, required Project flow consumers,
and SQLite Project flow/status read/write and admission in shared files. H7's
reserved files and draft remain untouched. An H7 handoff patch must be reconciled
with these edits rather than applied over them. The source is not ready for a
build while Chapter DTO/CLI/PM consumers still refer to the removed receipt API.

The public headless actual-CLI baseline failed at explicit headless filtering
(exit 2 after a successful agent launch). Exact receipt and fixture correction
are retained in `cutover/exec-ownership.md`.

## Ready for focused shared-Project proof

Shared production references to Chapter receipt APIs and ProjectFlowPlan have
been removed. Project flow/status persistence, Task admission, PM snapshot sync,
repository CLI routing, Rust/Swift Project summaries and current fixtures are
integrated. The exact H7 sync hook in `parallel-h7-shared.patch` is applied.
No claim is made that the legacy Project conversion, UUID contract, full Swift
consumers or execution-model cutover is finished.

The next Cargo batch uses the granted managed-worker slot, after resource
preflight, with the existing private-Home runner:

```
uv run python .lf/tmp/cut-i/run.py shared-project-api cargo nextest run -p loopflow --lib --test-threads 4 -E 'test(project_definition_updates_without_rewriting_the_task) | test(wave_reads_project_plan_and_preserves_unavailable_evidence) | test(chapter_deletion_stays_absent_from_wave_and_roadmap_planning)'
```

This is a compile plus three focused behavior checks, not an affected gate.
Foreign-file compile errors will be retained rather than overwritten.

`shared-project-api.log`: PASS, three selected tests, 1.355 seconds after 28.48
seconds compilation. Resource preflight passed at 66.1 GiB free. The compiled
source emitted three unused test import/binding warnings; those were removed
afterward. Swift syntax initially rejected indentation in two rewritten embedded
JSON fixtures, then passed after that fixture repair. Syntax is not type checking
or rendered proof. The managed Cargo batch is finished; no next batch is running.

Next managed batch (same integration slot, sequential):
`uv run python .lf/tmp/cut-i/run.py shared-project-dto cargo nextest run -p loopflow --test dto_fixtures --test-threads 4 -E 'test(pm_show_) | test(wave_detail_) | test(roadmap_)'`.
This validates the coordinated Project wire replacement and compiles the CLI.
Stacking handoff review found the exact later counterexample: its generic PR
writer compares parent equality, so it could refuse an already successful
publication observation. The patch is retained unapplied pending that repair;
no ownership or publication claim follows from its source-only review.

`shared-project-dto.log`: PASS, four selected Project/PM/status wire tests,
0.017 seconds after 21.88 seconds compilation; candidate CLI compiled. No Cargo
batch remains active. Next is the corresponding Swift DTO proof only:
`swift test --package-path swift --no-parallel -Xswiftc -gnone --filter DTOFixtureTests`.
Its log will be `.lf/tmp/cut-i/shared-project-swift.log`; it uses the same bounded
private-Home runner and serialized build slot. This is not rendered UI acceptance.

Swift DTO proof passed: 13 tests in 0.015 seconds (build and type checking
completed; existing weak-reference warning in ActiveRunsLifetimeTests remains).
This is package DTO proof, not Desktop rendering or retained-pane acceptance.
The stacking patch is now applied in main-owned source, with its generic parent
comparison removed and the stale publication test corrected to require both the
new parent and successful GitHub/Linear link observation. Its original patch
and source-only handoff remain unchanged as evidence. Focused stacking proof is
still due; no new batch is running.

H7 follow-up retains its reserved paths for UUID, absent-Task uncertainty,
archived predecessor and legacy adoption. Main retains the sole build slot.
Next focused command (stacking integration and publication preservation):
`uv run python .lf/tmp/cut-i/run.py shared-stacking cargo nextest run -p loopflow --lib --test flow_tests --test rebase_tests --test-threads 4 -E 'test(stacking_preserves_claim_and_history_until_the_owner_releases) | test(prepared_task_selects_parent_without_rewriting_work_or_publication) | test(existing_root_child_rebases_onto_parent_from_its_original_fork)'`.

`shared-stacking.log`: PASS, three focused tests in 5.840 seconds after 30.65
seconds compilation. This proves root parent selection preserving authored
bytes and independent publication facts, held-claim exclusion, and local rebase
from the original fork. GitHub is simulated; no configured publication occurred.
The batch is finished. Public headless Session admission/filter work resumes.

Next main batch: `uv run python .lf/tmp/cut-i/run.py conversation-admission cargo nextest run -p loopflow --lib --test session_cutover_tests --test-threads 4 -E 'test(interactive_session_is_rows_from_launch_to_completion)'`.
This compiles explicit conversation purpose/mode, pre-provider admission and SQL
mode/history/page filtering. It is not completion of the execution-model cutover.

Conversation admission's first compile failed: the added captured-launch argument
was missing in existing Flow/controller reservation callers. Those callers now
pass absence explicitly; mechanical steps still create no conversation. The
retained unwritable-store fixture now asserts rejection before provider start,
as required by the accepted new admission contract. Same narrow batch resumes
with both admission tests selected. The supervisor's concurrent first-Home
start-event loss remains an independent migration/admission obligation; no row
count or successful exit is accepted as proof of complete Exec evidence.

`conversation-admission-2.log` compiled, then both focused tests failed. Completion
still compared the retired `interactive` purpose in SQL; corrected to
`conversation`. The rejection fixture successfully refused both launches but
then queried an uninitialized database; it now initializes via the ordinary
inventory command before asserting zero conversations/work rows. Neither failed
result is a pass. Repeating only these two checks in `conversation-admission-3`.

Further Cargo is held until both reserved source contributions report ready,
per Jack's coordination steer. Repository/Task/search now precede SQL pagination;
admission captures canonical repository once, removing per-result Git scope reads.
Desktop reads subsequent pages. Added focused two-repository/worktree proof with
110 foreign rows and deliberately unreadable foreign payloads, and a Swift
101-conversation inventory proof. These are authored, not executed. Historical
unbound Session repository enrichment remains part of the offline import cut.

The already-built admission candidate passed actual Codex/private synthetic
upstream public headless launch/default-hidden/explicit-false/rename identity.
Receipt `.lf/tmp/execution-model/public-headless-admission/results.json` pins
SHA-256 `4d72eed3084c5b3bff8f77bfb919697d49d9e2a36ed9012a0735a74f82765eb3`.
These bytes predate current repository filtering and the completion predicate
repair. This is no public connect or configured provider acceptance.

Desktop offset paging was removed after review identified changing-inventory
skips/duplicates. Desktop explicitly requests `--limit 0`, one complete matching
SQL selection; CLI bounded pages remain explicit inspection pages. The new Swift
fixture uses 101 distinct IDs and then rename/insertion/completion, with no
additional snapshot owner. Source syntax passed before this revision; behavior,
EXPLAIN and dense latency remain due after source-ready handoffs.

Session inventory now emits only present SQL predicates rather than optional
OR guards. Repository/mode indexes can therefore constrain the matching set;
substring search remains substring evaluation, with no indexed-search claim.
EXPLAIN/latency still require execution. The public launch fixture now additionally
requires one actual nested lf child with the admitted Session/provider generation,
its creating Exec parent and released driver after completion. Production capture
now installs that frozen caller provenance for admitted conversations. This newer
fixture has not run against the newer source yet. Task/headless Flow admission,
retry provider generations and public connect remain pending in the same cut.

Both contributor sources are released. The new H7 adoption read/sync hook and
populated migration regression are applied once; original sync hook is retained.
Resource preflight passes (65.4 GiB free, own build 15.1 GiB). Next serialized
command: `uv run python .lf/tmp/cut-i/run.py integrated-init cargo nextest run -p loopflow -j 4 --lib --test-threads 4 --no-fail-fast -E 'test(development_initializers_recheck_after_another_writer_commits) | test(development_validation_keeps_one_snapshot_without_blocking_a_writer) | test(failed_first_draft_rolls_back_canonical_initialization) | test(expected_schema_reuse_) | test(backup_snapshot_and_migration_share_one_exclusive_transaction) | test(current_schema_does_not_take_the_database_write_lock)'`.
No full lifecycle or cross-Home global-absence claim follows from integration.
The first integrated-init invocation exited before compilation: nextest treats
`-j` as a test-thread alias, conflicting with `--test-threads`. The runner already
sets CARGO_BUILD_JOBS=4. Removed only `-j 4` and repeated the selected command.
