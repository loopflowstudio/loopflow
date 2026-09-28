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

`integrated-init-3.log`: seven focused initialization/validation tests PASS in
2.997 seconds, compilation 28.95 seconds. Earlier compile failed on a stray `repo`
field in a Cli test literal and an unused import; both fixed before this pass.
The pass predates the subsequent journal start-metadata edit.

Journal now supplies the observed process start independently of the event being
written. A late completion can retain that observation without inventing a lost
started event. A focused regression asserts both timestamps and the still-missing
receipt. Ordinary command logging remains best effort when the store is unavailable;
agent admission remains required before provider launch. Installation preflight is
unchanged. Next sequential proof: admission, scoped inventory and command lifecycle.

`integrated-admission.log`: seven focused checks PASS, including required-store
admission, main/worktree repository selection before paging, nested wrapper Exec
ownership, command failure and observed start metadata. Candidate is now rebuilt.
Next actual-CLI replay is `.lf/tmp/probe-first-home-execs.py --lf target/debug/lf`;
then the public Codex synthetic-upstream launch/nested-lf fixture. These use only
private Homes. No Cargo batch runs during those candidate proofs.

Actual first-Home replay FAILED on its first concurrent pair. Both commands
returned []/0, but session list warned `sqlite error: database is locked` and
lost its start event. Receipt:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-first-home-execs-1p2iyzed/receipt.json`.
The new schema tests do not close this real admission failure. Stop dependent
admission claims; inspect connection configuration and lock transitions next.
No warning suppression or row-count acceptance.
A 30-pair isolated sqlite connection probe reproduced 19 `database is locked`
failures at concurrent `PRAGMA journal_mode=WAL`, despite timeout=30 seconds.
This distinguishes a connection-level lock upgrade from the schema transaction
proof. The production opener now reuses the existing migration file lock only
when journal mode is not WAL, rechecks under that lock, and releases it before
schema migration. Already-WAL opens do not take that file lock. The backup path
also negotiates WAL under the same existing lock. No new durable owner/retry.
Next candidate compilation and focused preservation command:
`uv run python .lf/tmp/cut-i/run.py wal-admission cargo nextest run -p loopflow --lib --test exec_ownership_tests --test-threads 4 -E 'test(inspection_records_one_completed_exec_without_starting_work) | test(backup_snapshot_and_migration_share_one_exclusive_transaction) | test(development_initializers_recheck_after_another_writer_commits)'`.

`wal-admission.log`: three focused checks PASS in 4.926 seconds, compilation
35.61 seconds. Actual first-Home replay now PASS on all three fresh-Home concurrent
pairs: each command has matching start/completion receipts, two succeeded Execs,
zero agent Runs and no warnings. Receipt:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-first-home-execs-1dmf_kwt/receipt.json`.
The preceding failed receipt remains evidence. This proves these six real local
commands, not arbitrary fleet load or installed-Home acceptance.

Publication patch is integrated with its invalid intermediate-state ordering
corrected: reconcile a known identity's pinned merge intent before replacing its
head, then persist identity before Linear linking/readiness/follow-up reads. First
attachment has no merge intent and saves immediately. Validation stays unchanged.

Public headless nested-lf proof FAILED: launch and both inventories succeeded,
but no via_agent child was found (candidate d29d1d2573435a7271ca016b6d85756c54c40dccba6ff85dc0f2e0a7505d9c13,
`.lf/tmp/execution-model/public-headless-parent`). Synthetic provider requests
show the actual child command returned []/0; provenance is not established.
The existing feasibility probe set per-thread shell environment, whereas normal
Codex launch only set engine environment. Production now supplies explicit launch
provenance and selected executable/Home through the thread's shell config. This
is a source hypothesis until the same public fixture passes. Fixture now retains
observed SQL provenance before its assertion for useful future failures.

H7 integrated proof ran 13 tests: 12 PASS, archived-predecessor comparison FAIL.
The mismatch is only fractional constructor timestamps versus stored seconds;
fixture now reads its pre-action Task from SQLite before checking preservation.
The twelve-provider-mutation two-Home matrix and legacy lost-response matrix
passed (59.178s and 22.252s respectively). These preserve the stated missing-local
Task and cross-Home limits; no configured Linear mutation ran.

Per-thread environment source review found cloned launch.env could restore keys
removed from the engine command. Corrected to select sanitized explicit command
deltas for intended launch keys, honoring None removals and fresh Home overrides;
account environment is not forwarded wholesale. New child-behavior test includes
forbidden keys, private development Home and explicit release-alias removal.
Publication readiness proof now starts with a differing head-pinned request to
exercise validation/reconciliation before the early identity save.
Next sequential command combines these focused checks with the publication
read/readiness cases; it rebuilds the candidate for the public nested-lf proof.

Publication preservation final focused command: `publication-preservation-3.log`
records 2 PASS (3.284s): acknowledged creation survives follow-up read failure,
and differing-head known identity survives readiness failure. The preceding
failures were fixture Project JSON (`flows` instead of required `flow/status`)
and missing pinned presentation; both are retained in earlier logs. Existing
changed-head revocation, sanitized child environment and archived predecessor
preservation passed in `integrated-publication-env.log`. No remote cause claim.

Public nested-lf failure narrowed on unchanged candidate 38f775a3: parent-4
fixture-only journal debug records admission EPERM; parent-5 also observes
`ps` rejected by the unchanged workspace-write sandbox. Admission required OS
start inspection before creating its context, so no SQL write occurred. Runtime
now records observed command entry independently; unavailable OS start remains
None, gives no process-owner identity or receipt. Conversation driver naming uses
actual Exec identity, not OS signaling evidence. No sandbox widening.
`sandbox-journal.log`: 3 focused checks PASS (4.034s; compile30.55s).
Actual public probe parent-6 PASS, candidate SHA
3847c4c353b38841c70f23214667594b47f36a4057e0a80c7a54ebf01d25f451:
real Codex + synthetic Responses, private Home, launch/discovery/rename and
nested child's via_agent/parent/Session/provider-generation persisted. This is
not public handoff or real-model continuity proof. Debug diagnostic and failed
receipts retained. Current scope still requires physical lifecycle conversion.

Import review read in full. Its preservation counterexamples remain required
before removing historical owners; especially unknown process start, complete
non-review Flow captures, conflict detection and historical attribution.


## Native socket and command entry integration — 2026-09-28

Normal Codex launch now uses its private Unix WebSocket. Agent tool provenance
is thread-specific; the engine receives no inherited conversation authority.
Established managed driver teardown detaches, without a group kill that could
race a new driver or terminate a sibling. Explicit native writes remain fenced
at dispatch. Engine cleanup and public restart remain unfinished.

`native-socket-admission-2.log`: four PASS (6.379s, compile32.98s): helper
admission, H7 authored content preservation, fresh conflicting status refusal,
and actual CLI directory-setup plus append-file obstructions. File journal
failure no longer suppresses SQL command start/completion; obstruction retained.
The H7 fixture correction adds a Notes heading, preserving KR semantics without
changing the parser. Earlier successful mutation/retry matrix remains recorded.

`public-connect-2/results.json`: real CLI connect + actual Codex engine,
synthetic Responses and a controlled native-protocol client PASS. It rejects
retained-client interrupt and preserves active selected/sibling turns and provider
generation across transfer. It does not prove native UI, old headless-driver
teardown, restart or recorded completion/usage. Jack's supervisor correctly
narrowed its child claim: latest via_agent alone could select a sibling. Fixture
now uses a harmless distinct sibling command, snapshots Exec IDs, checks exact
new child Session/generation/driver, and rejects old start/steer too; replay due.

`entry-and-native-safety.log`: four assertion passes (4.827s; compile31.52s),
including failed explicit Wave resolution and nested runtime ownership. Nextest
labels the existing exact_process_evidence test LEAK; this is not clean process
settlement proof. Supervisor independently replayed the original failed-Wave
case on copied SHA e224799d: succeeded inventory plus failed unknown-Wave Exec,
unchanged error text. Receipt is in the Task steer. Installation and screenshot
bootstrap remain outside ordinary store admission; help/parse/current-directory
and unavailable-store paths cannot be described as complete command coverage.

Handoff history review read. Live reconnect currently has no durable completion
or usage ingestion after the original capture closes. Driver failure must remain
Exec evidence, not overwrite a later native success. Next owner cut is Session
history with exact native turn receipts and its public history/usage consumer;
Flow consumption must then reference that success. No history acceptance yet.


Public native history first consumer is implemented: `session history ID --json`
selects Session receipts by sequence in SQL; normal Codex and connect use one
receipt writer, with thread/turn/kind identity and conflicting-completion refusal.
Snapshot completion retains unknown initiating Exec/generation and does not mint
start or usage. Correlated turn/start replies retain their initiating Exec;
that differs from the later driver which owns a nested command after handoff.
Native lifetime usage counters remain explicit, not mislabeled turn spend.
Old Run usage/Flow consumers are not yet converted; no whole-owner cutover claim.

`public-connect-history-1` PASS, copied SHA
351fad821e093ca2b1bf3825a654373f054002865a20421f260ccaee5cfce694:
real Codex, actual CLI connect/history, synthetic nonzero Responses usage and
controlled client. Exact selected child Session/generation/new-driver assertion
passes with a distinct harmless sibling command and Exec-ID snapshot. Old client
start/steer/interrupt are rejected. Both initial and continued completions and
four cumulative usage receipts persist; repeated native read adds no duplicates.
It proves neither rendered UI nor no-client usage recovery, public approval,
old headless-driver teardown, restart, normalized spend or Flow settlement.
`conversation-history-focused.log`: 2 PASS1.308s, compile29.95s (missing start/
usage and conflict preservation; Rust DTO roundtrip). Swift DTO proof running.

Supervisor's native approval probe is read: Codex replays a pending approval
while B resumes before A disconnects. Public fence proof remains owed. Native
turns have completion timestamps but no usage field; paginated recovery and
retaining missing usage remain required. No extra approval owner is selected.

Control-path note: Jack's supervisor reports the active installed selection
changed. Use the captured installed lf and original Home supplied in the steer
for any later Task/status/checkpoint operation. No such operation ran here;
source proofs still use only disposable Homes.


Swift Session history DTO fixture PASS (1 test,13.57s build). Native recovery now
walks thread/turns/list ascending cursor pages with items not loaded; this is
explicit connection recovery, not list-time history I/O. More-than-one-page
behavior still needs a focused proof. Ordinary unchanged receiver histories
remain idempotent; no per-turn usage is inferred from a completion page.

`public-live-history-1` did NOT complete. Its normal connect/history phase passed,
but the live-headless case skipped controlled-client request 0 and waited forever
for response 1. Fixed that fixture sequencing error. Cleanup also raced private
provider filesystem shutdown; exact PID/start/group cleanup now waits for observed
exit and reports force termination. Neither failure establishes live handoff.

Generation review correction: transport generation is removed from observation
insertion. Receipt ancestry/generation reads the same turn's correlated start;
a resumed old receipt cannot take a replacement engine's generation. Unknown
origin stays unknown. The focused replay fixture includes a gen1 start, provider
replacement to gen2 and delayed usage. Command `native-history-origin` running.

Jack's supervisor independently proved public driverless completion/usage replay
on the earlier candidate. Useful assertions are integrated into the main fixture:
all lf clients exit130 before native completion; no completion before reconnect;
one success afterward; original interrupted Exec and earlier history unchanged;
known cumulative usage preserved, repeat read unchanged. New integrated replay due.


`native-history-origin.log`: 2 PASS1.302s, compile30.17s. Delayed replay after
provider replacement retains original generation1/Exec, and unobserved turns
remain unknown. Observation no longer supplies transport generation.
`public-live-history-2` PASS, copied SHA
d6ef27459f35fefa6e8540c7242fae373ce1c80bd47a61b26af7bd4bd7f8d317.
It covers corrected exact child provenance; public old-client start/steer/interrupt
rejection; active turn and sibling surviving old headless SIGINT/exit130 after
transfer; provider generation retained; native completion/usage persisted with
original turn origin. It also includes the supervisor's no-lf-receiver interval:
both connect Execs interrupted130, zero completion before recovery, native turn
success, reconnect records one success plus reported cumulative120/30 usage,
prior history unchanged and replay identical. Fixture-owned engine cleanup now
waits for exact process exit and completed without force termination.

Limits: real Codex + synthetic upstream + controlled native client, not rendered
TUI/Desktop or configured model. No public approval fence proof, restart, >100
native-turn recovery proof, normalized usage, physical Run removal or Flow
consumption follows. Existing Run consumers are still outstanding. The next
major owner conversion must now follow Jack's requested coherent checkpoint and
local upstream rebase. Cargo formatting/static checks are in progress; no delivery
or installed-Home migration is authorized.

### Pre-integration checkpoint, 2026-09-28

Formatting, all-target Clippy (-D warnings), architecture inventory and fixture
Ruff pass. Clippy receipt: .lf/tmp/cut-i/native-history-clippy-2.log.
Migration history check refuses missing upstream 0.12.24.001_release.sql;
this is retained upstream drift, to reconcile through the local rebase.
Observed base 5bdcef6b65419d5db2583ee791cede2f1a95b3df; origin/main
c512813b5b333f2ae012503b1781fcbd458be67a before fetch.

Jack's supervisor supplied additional actual public CLI/Codex proofs on the same
d6ef2745 candidate, with synthetic upstream and controlled client:
- supervisor-public-approval-1: stale approval has no effect, current approval
  executes once, sibling survives, native success persists.
- supervisor-pagination-1: baseline plus 103 missed turns recover 104 exact
  completions across cursor pages, old history/repeat unchanged; 103 origins
  remain unknown. Only latest missed turn has resumed usage (last20/5,
  lifetime2100/525); 102 missing usage receipts stay absent.
- supervisor-zero-clients-1: both public clients exit130 and inspector closes;
  agent writes finished-alone before a fresh inspector connects. Recovery keeps
  one success, cumulative120/30, old history and interrupted starting Exec.
  Marker ordering does not prove the entire turn completed before reconnection.

These supersede the corresponding narrow proof gaps above; they do not establish
restart, normalized per-turn usage, Flow consumption or physical owner conversion.
Useful approval/pagination assertions remain to integrate into the maintained
fixture after rebase. No source/build changes accompanied supervisor proofs.

Jack requested regular rebases; the supervisor corrected the spelling to
lf rebase --manual (local), because bare rebase can push.
Use the captured installed control binary/Home. Preserve the saved invocation:
upstream catalog changes cannot grant this Run publication authority.
Semantic integration review must retain upstream provider-account selection/
missingness and clear inherited LF_AGENT_CALLER plus copied native driver/engine
authority when changing Home, preserving history in two disposable stores.


### Post-rebase checkpoint preparation, 2026-09-28

Local rebase completed at ce97003cf, based on c512813b5 (78 ahead / 0 behind
at observation). No publication occurred in that integration. Migration history
now passes: 55 released migrations through 0.12.24, 19 drafts. The read-only
projection audit was read in full; its Wave enablement finding matches the
coordinated Rust/Swift/fixture repair. Remaining history/Run/Flow and off-roadmap
Task breadcrumb work is still part of the physical owner conversion.

`rebase-focused.log`: 20/21 PASS, compilation32.41s/tests9.675s. Passing checks
cover copied history without driver/socket authority in two private stores,
LF_AGENT_CALLER scrubbing, account-before-native-ID and Task event observations,
retained captured Flow behavior, acknowledged publication/readiness failure,
and restoration of Task checkout history. The DTO failure was its stale assertion
that status has no Projects field; H7 now supplies Project history. The assertion
is corrected to retain status/stranded Task and Project evidence, pending rerun.

Jack's supervisor supplied a strict RED public busy-turn continuation proof on
d6ef2745: B's turn/start returns A's same native turn, but origin reassignment
fails before forwarding the reply. Direct native comparison confirms same-turn
success. The repair correlates turn/start requests/replies with newly observed
native turn/started, preserving an existing or unknown origin. Conflicting
actual origin receipts still fail. Both message orders and input after an
unowned start have an authored regression; behavioral execution remains due.

Jack has authorized publication of the next coherent verified checkpoint before
physical owner conversion. This supersedes the local-only publication hold for
that checkpoint; it does not authorize landing, installation or real-Home
conversion. Keep scratch and the saved Flow definitions. Main owns all executable
changes, builds and Git; all bounded contributors/probes are terminal.


Verification for this checkpoint: `rebase-dto-busy.log`10/10 PASS (31.60s
compile,1.423s tests); `rebase-swift.log`47/47 PASS in four selected suites,
including full101-Session inventory through rename/insertion/completion. This is
inventory, not retained pane replacement acceptance. `rebase-clippy.log` all-target
Clippy -D warnings PASS19.20s; formatting, diff whitespace, architecture coverage
and released migration history pass. The earlier20 passing Rust checks remain
valid for their unchanged behaviors; no full gate is claimed.

The strict actual public busy-turn regression passed on copied SHA
f2b9f5ab7113cf4c5b1a08017927c791ad79d7da51f09c0e584eda10187600b4,
`supervisor-busy-start-regression-2/results.json`. It retains original origin,
returns the same active turn to the new driver, and records one success plus usage.
Real Codex/synthetic upstream/controlled client/private Homes. The focused unit
proof additionally checks both fresh-start message orders, unknown prior origins,
and rejection of contradictory actual origin evidence.

Prepublication remote inspection: main c512813b5, branch d07e56933, GitHub#1296
open with no auto-merge request. The pinned Task reader retains current implement
Run and13-step saved invocation; no catalog recompilation. Existing top-level
*-review.md reports moved into scratch/reviews/ with references updated because
the captured publisher predates removal of that broad gate-artifact deletion.
Evidence is retained; no source behavior changes accompany that relocation.

Remaining: public restart, physical AgentSession/FlowSession owner conversion
and Run removal, import preservation, exact Flow success consumption, normalized
usage and Desktop history/off-roadmap ancestry/pane retention, final dense
measurements and concept review. Configured provider/Desktop, real-Home migration
and release are not established by this checkpoint.

### Published checkpoint and canonical CI repair, 2026-09-28

Published e13f299093d2234b3728a8063ad310fd212f62eb through the captured installed
CLI/Home. Remote branch, GitHub#1296 and the Task's publication.github.head_sha
agree. Base remains c512813b5 (79 ahead/0 behind at publication), no auto-merge,
working tree clean afterward. Saved invocation1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c
still selects implement in its13-step captured Flow. Readbacks remain under
.lf/tmp/rebase-integration/. No landing or installed-Home conversion occurred.

CI run36482349277 exposed the canonical empty-draft initialization path nesting
migration transactions. The physical conversion is held until this is repaired.
`canonical-init-red.log` reproduces the exact error locally with an explicit
empty manifest. `canonical-init-repair.log` passes4/4 (compile30.91s/tests3.016s):
fresh in-memory/file initialization, populated canonical upgrade with retained
previous-generation backup, retry, concurrent draft initialization, backup/writer
exclusion and current-store lock behavior. The repair keeps one transaction
owner; canonical file migration prepares WAL and its lock before entering it.
The three exact CI controller failures still require the materialized-source
replay. No draft checksum was changed.

The Rust installation fixture no longer requires the removed --daemon-target;
retained CLI target, Home, preview and unchanged-store assertions remain. Its
real installation proof requires the disposable container harness, never this OS
account. Python release contribution has been read and accepted:4 focused
simulated installer checks pass; source assertions still require candidate-owned
promotion/CLI target and prohibit direct activation. Website contribution remains
independent; main replaced its separately owned obsolete portable-HTML Run phrase.

Resource preflight fell to60.2GiB free below64GiB. Builds held. The prescribed
--recover launched uv cache prune under uv run and remained pending; lsof showed
both owned parent66562 and child68679 opening the same cache lock. Main terminated
only child68679 to release this attempt; retry without an enclosing uv run is
pending. No shared live source, installed state or provider was targeted.

The same recovery attempt subsequently completed (exit0), so the proposed retry
was unnecessary. Cache pruning failed without deleting cache bytes; the resource
script reclaimed allowlisted inactive build artifacts and reported65.9GiB free.
The canonical-materialized7-test replay is running in a disposable source copy
with inherited authority removed. No product build was launched before recovery.

Website handoff read in full after its writer returned. Main's final portable
HTML check passes1/1 in0.82s (`portable-html-repair.log`), including generated
freshness, title, canonical execution link and standalone stylesheet behavior.
The contributor's security browser pass remains scoped to its unchanged bytes.
The resource-recovery contributor owns scripts/resource_envelope.py and its
Python tests; main does not edit or checkpoint that active contribution.

Canonical replay is green: `canonical-materialized-repair.log`,7/7 in4.373s,
compile31.14s, all19 drafts materialized only in the recorded disposable copy.
It includes all3 exact failing CI controller tests and the4 migration checks.

The disposable-account installation harness passed3/4, including retained-pair
compatibility. Fourth test failed at its initial Task run because tmux is absent.
Trace: ensure_flow_position calls park_at_review on a human boundary with no
pending Session, before launch_task_process's is_human return. Park reserves an
unpublished review; prepare launches its terminal because session_run_id only
returns published work. Production behavior is intentional, not an autonomous
cursor mistake. The fixture now seeds the existing published review it intends
to complete, using normal reservation and explicit synthetic publication; no
terminal/provider is launched. Branch-only feedback is written through the
existing ready_session operation. Its installed authority, stale token, retry,
branch isolation and single-consumption assertions remain; rerun pending.

Container cleanup completed on the failure. The source snapshot predates removal
of one unused `complete` binding found by its compiler. Disk fell below64GiB
after the builds; recovery is running from the activated virtualenv without an
outer uv process before any further product build.

Installed-readiness repair passes in the disposable Docker account:
`task-review-installation-final.log`,1/1 in17.31s after23.69s compilation.
The preceding targeted rerun reached stale readiness and failed only at an old
error string; the final fixture requires current `Session is stale` and unchanged
saved Flow. The first3 installation checks passed in the original run; no broad
repeat was needed. Containers were removed after each run. These are real CLI
installation-routing proofs with synthetic published review evidence, not an
actual provider/terminal launch or installed-host acceptance.

Resource handoff is read and accepted:6 focused tests plus Ruff/format and a
private real uv reproduction. TESTING.md now names the15s timeout and retry
condition. Main's real recovery used this bounded path, timed out explicitly,
and continued;64.6GiB free was observed. No global-cache-prune success is claimed.
The next preflight passed64.4GiB; all-target Clippy is running serially.
Canonical history check retains55 shipped migrations and19 drafts; architecture
inventory passes its declared coverage. Full physical owner conversion, restart,
import/Flow/usage/Desktop acceptance remains outstanding.

CI-repair all-target Clippy passes (`ci-repair-clippy.log`,16.70s); formatting
and working-diff whitespace pass. All contributor ownership has returned. This
checkpoint includes the canonical transaction repair, installed-readiness fixture
reconciliation, stale installer/website assertions and bounded cache recovery.
It does not establish full CI, physical owner conversion or code completion.

Repair checkpoint2958d289c was published to existing PR1296 under Jack's renewed
publication direction. GitHub head, remote branch and Task publication and
presentation heads agree; basec512813b5,80/0 ahead/behind after conflict-free
manual rebase. Auto-merge remains absent. The saved invocation is unchanged.

Physical conversion began afterward: AgentSession and FlowSession Rust names,
in-place agent_sessions/flow_sessions table rename with unchanged historical
migration bytes, current SQL consumers and a populated preservation regression.
This is retained intermediate work, not Run removal or cutover acceptance.
The next focused command is held by resource preflight62.1GiB/64GiB; no build is
running. Main is removing only this checkout's rebuildable loopflow package
artifacts with cargo clean -p loopflow, retaining dependencies and proof receipts.

Owner rename proof passes3/3 (`renamed-owners.log`,28.87s compilation,1.374s
tests): populated captures/claims/feedback/native history unchanged, no parallel
old tables, and copied-store connection/driver exclusion. Branch copying still
recognizes the historical table spelling only to remove copied operational
authority before its one-time upgrade; current readers use the renamed owner.
Physical names and Rust types are changed without aliases. Existing Run fields
still require relocation. The five selected actual-CLI Session/Flow proofs are
running; this is not cutover acceptance. Package cleanup removed13.2GiB of this
checkout's artifacts; subsequent resource preflight passed75.2GiB.

The five CLI selections first returned2pass/3fail: two obsolete count("sessions")
assertions and one stdio-only shared Codex stand-in. Updated counts use the
physical owner and the shared stand-in accepts the advertised Unix WebSocket.
The four affected CLI assertions then passed, with one Nextest LEAK retained in
renamed-owner-cli-repair.log. The socket stand-in now ends its own client and
child after the synthetic completion; provider-fixture-teardown.log passes both
Flow tests without a leak (16.791s). Production engine survival is unchanged.

Hosted CI executable lookup failures are repaired under TestLfBinGuard, including
the two additional reachable review fixtures identified by the supervisor.
review-executable-pins.log passes4/4 (31.53s compile,1.572s tests). No production
executable fallback was introduced. Shared mock transport is synthetic evidence,
not a new live provider result. This remains an intermediate checkpoint; the
next owner conversion moves ancestry and prospective bind off historical Runs.

Ancestry work is unfinished. The first compile found a typed ID conversion and
a breadcrumb fixture still taking Run; both were repaired. The supervisor
identified an incorrect new test assumption: first bind DOES start a Task, per
Jack's retained decision, even when usage binding is prospective. Corrected the
new Session assignment triggers and test to set Started once at bind without
rewriting old Run/event attribution. No passing proof or product approval follows
from the withdrawn future-work-only expectation. Native child/Ask inheritance
now carries the admitted Exec into the database transaction (not thread-local
context read after spawn_blocking); current provider generation is checked.

conversation-ancestry.log passes3/3 (32.06s compile,16.340s tests): first bind
starts once without changing prior work; native pre-bind/active-turn receipts
retain attribution; actual CLI child launch and Ask inherit current assignment
from synthetic admitted provider provenance, including a done Task. Stale
provider launch is rejected before reaching the stand-in; a separate untouched
Task remains unstarted after observation. This is not a live-engine bind proof.

The broader migration/Session check stopped at a malformed populated fixture and
five old Ask fixtures whose caller directory was outside Home/runs. Branch
isolation correctly scrubbed those callers. They now use a canonical fixture
Run directory with inherited LF authority cleared. The first Task-worktree
fixture repair was insufficient: the Flow stores NULL cwd (Task owns it), and
its capture needs an id plus positive position_version. All three fixture setup
failures remain in ancestry-migration-and-sessions*.log / ancestry-upgrade.log;
no production validation was weakened. The corrected populated upgrade now
passes inside ancestry-upgrade-and-remaining.log; remaining affected tests are
running without fail-fast so later counterexamples remain visible.

The remaining reader assertions now pass: ancestry-readers-repair.log retains
one pass plus the next stale import kind expectation; ancestry-import-repair.log
then passes the complete imported identity/name/capture/outcome proof (8.343s).
The earlier wider run finished21/23. No preservation assertion was removed:
headless conversations now have Sessions, macOS paths compare canonically, and
the public conversation kind is explicit.

canonical-ancestry.log passes4/4 on an isolated materialized source copy
(27.24s compilation,2.053s tests): populated ancestry and Started preservation,
owner rename preservation, prospective usage binding with first-bind Started,
and canonical empty-draft initialization/upgrade. The source hash inventory is
.lf/tmp/cut-i/canonical-ancestry-source.json. Released migration history passes;
portable HTML/freshness passes1/1. These are disposable fixture proofs, not an
installed migration or complete historical import acceptance.

Checkpoint review: Session list/bind/breadcrumbs now consume AgentSession
assignment directly; bind_session_runs_in and its historical Run rewrite are
deleted. New independent child launches/Asks resolve current assignment from
admitted caller Session/provider generation inside admission, while earlier
native events retain their original attribution. New ancestry SQL adds ownership
validation and first-assignment Started without a synthetic Run. Relative to
d8665a753, production Rust prefixes before cfg(test) plus SQL initially measured
+372/-60 (integration tests, test tails, docs/generated/scratch excluded).
Clippy then found the larger AgentSession made SessionTarget unbalanced; its
existing row variant now boxes the Session alongside Run. No policy changed.
Run/current_run_id and headless Flow admission remain unfinished; this checkpoint
is explicitly not a completed owner cutover.

Final ancestry formatting and all-target Clippy pass (ancestry-clippy-2.log,
14.71s). Fresh remote inspection observes main a2b59ed50666a433c97cb9002476e74351723759
and the Task branch2958d289c. Jack's standing checkpoint publication request
applies; local commit then manual rebase precede publication. No landing,
installation or saved-Flow replacement is selected.

Manual rebase onto a2b59ed50666a433c97cb9002476e74351723759 completed,
82/0 ahead/behind. Conflicts were catalog docs and the queue tail assertion;
main's retired memory skills and rebase/realign queue remain. The Task status
receipt still names invocation1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c,13steps,
implement index2/iteration9, Run8e4800ebef6045699e0be1fac668e6cb. No saved
invocation was recompiled.

Supervisor found Import::start still inferred Started from Task Run membership.
The new actual-CLI bind-then-import fixture reproduced this on dry-run (wrong
nonempty tasks_started) in import-started-red.log. Import now reads the existing
Task Started column. import-started-and-rebase.log passes3/3 (29.65s compile,
8.918s tests): bind-first dry/real import retains Started and reports no new
start, untouched first import still reports its Task, and the reconciled catalog
Flow reaches its delivery boundary. No provider or publication occurred in the
fixture. The physical owner/Run-removal work remains incomplete.

Post-rebase all-target Clippy passes15.06s (ancestry-rebase-clippy.log); formatting and whitespace pass.

Published checkpoint705645cd1ad36808460e8f0e9e4767d1d8cc7191 to existing
PR1296 after manual rebase. GitHub head, remote branch, Task presentation and
publication heads agree; basea2b59ed50, auto-merge absent. The same saved13-step
invocation and implement Run remain. Evidence retained in
.lf/tmp/rebase-integration/task-after-ancestry-publish.json.

Next cut: headless Flow reservation lacked an AgentSession. The focused
flow-conversation-red.log reproduced that missing admission; reservation now
creates the conversation before launch and reuses it for a retry at the same
captured node/iteration, retaining name and failed Run history. Mechanical steps
remain without agent conversations. flow-conversation-admission.log passes1/1
(26.48s compile,1.306s tests). Managed Task launch now claims the same driver
before provider launch. Public CLI failure/retry and taskless-decision proofs
are running. This is unfinished: native retry continuation, exact native success
consumption and physical Run removal are still due, not proved by this store test.

flow-conversation-cli first passed taskless decisions but failed Task-attributed
headless membership: surface still treated every Flow conversation as a review.
It now reads the stored Flow and retained node/iteration for current/earlier/past
membership; the replaced review-only helper is deleted. flow-conversation-and-ci
passes4/4 (29.62s compile,11.780s tests): actual CLI failed/retried conversation
identity/title, two hosted ad hoc/replay fixtures, and inaccessible-store refusal
before provider launch. Successful unplanned work uses an empty writable private
registry, not unrecorded launches. The compiler's unused old membership helper
warning was then removed by deleting the helper.

Native taskless retry: .lf/tmp/execution-model/native-flow-retry-1/results.json
passes on copied binary SHA a4fcad5a4d948fd9fdc5437b6afd0d3eb6f794eb2161fbf7e73149ef93dca8c9.
Actual Codex0.157.1 receives a synthetic Responses failure, then public Flow
resume --retry succeeds. Session, endpoint, native thread, provider generation
and origin remain identical; old history is byte-equivalent, failed/completed
turn IDs differ, and the new nested lf Exec follows the retry driver. Exactly
one fixture engine was launched and exact owned-engine cleanup completed. This
proves the live-endpoint taskless case only: managed-account/native-Home
continuation, dead-engine recovery, busy surviving turn and exact Flow history
selection remain required. Stored endpoint is not liveness, and this passing
probe does not supply it. No configured provider, installed Home or new
publication was used.

The supervisor's engine-loss probe made the stored-endpoint failure concrete:
public retry returned ENOENT after exact fixture-engine termination. The native
connection now retains PID/start evidence on AgentSession separately from driver
lifetime. A known dead/replaced PID permits a fenced new provider generation;
unknown process evidence does not. Replacement clears transport/process evidence
while preserving the native thread. A copied Home clears these process facts too.
The tracked --flow-engine-loss fixture resumes the same thread, retains prior
history and records its successful turn under the replacement generation;
native-flow-engine-loss-1 passes on 2b07fbcb7a30905c7e0b932ec92f60731ffd8625419841261ba74cd8e9835cf3.
This is actual Codex with synthetic Responses in a disposable Home, not configured
account or Task-worker recovery. No old engine was signaled by production code.

conversation-engine-ownership initially failed compilation because the removed
review-only membership helper retained a test caller. After repairing that test,
conversation-engine-ownership-2 passed4/4 (26.12s compile,1.373s tests), including
copy isolation, replacement fencing/history, Flow reservation and nested capture.
All-target Clippy passed14.91s before the subsequent graph-projection regression.
The supervisor correctly distinguished nested capture from Session navigation:
numeric stored node IDs were incorrectly stringified into structural wire keys.
A new stored-Session projection test now covers nested/post-XOR current and earlier
occurrences; its red/repair result will be recorded below. Wire meaning remains
structural until the coordinated numeric Rust/Swift graph conversion.

Driver-loss remains an actual failing public recovery case:
supervisor-flow-driver-loss-1 retains the live engine but Flow waits for an absent
Run completion. The next dependent owner conversion must select exact Session
turn history under Flow/version/claim authority and retain the unknown command
outcome. It must not synthesize a Run success/interruption from process death.
TaskLauncher unconditional input and current-account selection are still open;
these tests do not establish busy-turn recovery, recorded native Home/account
continuity, exact Flow history consumption or complete Run removal.

The stored Session navigation proof first failed with node3 instead of1/fix/1
(stored-session-graph-red.log). The projection now resolves the stored preorder
ID through the captured graph before emitting the existing structural wire key.
stored-session-graph-green passes1/1 (26.45s compile,1.345s tests), exercising
nested and post-XOR Sessions as current and earlier occurrences with retained
iteration tuples. This repairs the current wire contract; it does not implement
the final numeric Rust/Swift graph DTO conversion or prove rendered navigation.
Native live retry and engine-loss retry both passed again on the final thread
identity guard in native-flow-live-final and native-flow-loss-final. A subsequent
source safety review added a connection probe before replacing a dead launcher's
provider: a surviving native endpoint retains its generation, while ambiguous
connection errors cannot authorize replacement. Final build/replay remains due
for that last change.

The projection regression passes after materializing all22 drafts into a canonical
0.12.25 batch in a disposable source copy. canonical-engine.log passes4/4
(26.79s compile,2.004s tests): stored graph navigation, retained native history/
fences, copied-Home authority exclusion and empty-draft initialization/upgrade.
The source fingerprint is canonical-engine-source.json beside that log. Final
all-target Clippy passes14.92s after moving the environment-guarded test's async
body into its explicitly owned runtime; the prior Clippy failure was an
await_holding_lock test warning, not a runtime proof failure.

Supervisor independently proved recovered completion origin on candidate4ac6ec13:
supervisor-flow-recovered-origin-1/results.json deliberately removes the SQL
completion after a real failed turn, retaining its start. After exact engine
termination/retry, native recovery restores the old failure with generation1/
original Exec; the new success names generation2/retry Exec, and repeated public
history reads match. This uses real Codex, synthetic Responses and a simulated
missing SQL receipt; it is not an observed database write failure, managed account
acceptance or missed-usage proof. Driver-loss and exact Flow consumption remain.

Final checkpoint engine-loss replay passes on709b1ca131aa7c39f7b26ddf3fea8f5139a42c1122a5dc6a5c02d7f22efabdae
(native-flow-checkpoint/results.json): controlled failure1, retry0, same native
thread and Session, old history retained, one replacement engine/generation.
Formatting/whitespace pass and main remainsa2b59ed50 by fresh ls-remote; the
Task branch remains705645cd1 before this checkpoint. Scope review found no new
product owner: new PID/start facts are AgentSession operational evidence, cleared
when copying stores; native thread is independent of socket lifetime. The current
wire graph key remains a projection, never stored Session ancestry. Run-backed
Flow settlement/account continuation remain known unfinished behavior.
