# Data model storage slice review

## Current review: invocation retention

2026-09-26 · LOO-298 · review-slice

Reviewed HEAD `54abb81ff5deb165bc4e39473766efa53220d65b` against Task PR base
`a48eeb6aed383123c51a6c6cc673591ae19b1417`; the working tree was clean.
Scope: the complete branch change, with particular scrutiny of invocation
retention at `60639b3e6`. Governing intent remains the
[amended design](data-model-one-table-per.md),
[review feedback](data-model-review-feedback.md), and
[Session ownership decision](session-runs-and-current-run.md).
The later participant's approval (name unresolved) supersedes the older Task
title: taskless Invocations and Session owning Runs plus a current Run.

**Disposition: not ready to publish.** Retaining Task invocations advances the
approved architecture without adding a second Task writer. It does not complete
the shared Invocation/Run/Session cutover and is explicitly not an independently
publishable slice. Behavioral preservation remains unexecuted. This review
changes only this note; no PR, Task, Home or Flow navigation mutation occurred.

### Findings

1. **The retained invocation is not yet a stable conversation owner.**
   Completion now retains capture, cursor, pending Run, feedback and final claim
   in `flow_invocations`; restart and authorized reopen mark the old row
   replaced. However, Session discovery and exact review lookup still enumerate
   current Task invocations and decode each one before selecting human steps
   (`store/sqlite/durable.rs:392`, `ops/human_session.rs:1166`). An unrelated
   malformed current capture can still prevent reaching a valid Session.
   `open_boundary` still clears Ask feedback at `human_session.rs:780` and Task
   review feedback at `:799` before replacement. Invocation retention does not
   address either counterexample from the previous review. These are
   source-traced findings, not executed reproductions in this review.
2. **The implemented lifetime boundary is coherent at source level.**
   The partial index selects one current invocation per Task. Current writes
   and claims select `state='current'`; unclaimed position updates also match
   invocation ID. Human completion reads and compares the entire expected
   position inside an immediate transaction before closing it; worker completion
   matches the serialized claim, including invocation identity. Chapter reads
   retain historical execution while excluding closed claims from current
   worker activity. No bounded source defect was found requiring another code
   change in these paths. This is not a behavioral pass.
3. **Verification is still blocked by the measured resource envelope.**
   Preflight and safe recovery both fail at active
   `jack-heart/main-view-task`: **15.3 GiB / 12 GiB**, with **98.8 GiB** free.
   Recovery preserves the active foreign build. TESTING.md's
   [resource rule](../TESTING.md#bounded-and-honest) says unresolved pressure
   stops product tests. Therefore neither focused Rust execution nor disposable
   materialized migration proof ran. No installed binary or private Home was
   used as a substitute for the changed code.

### Evidence matrix

`pass (source)` describes inspected code only. Every full-design acceptance
claim remains open; authored or previously compiled tests are not executed proof.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: retain invocation lifetime | Completion/restart preserve execution and review history | One `flow_invocations` table; close instead of DELETE; identity matches capture JSON | Draft `retain_flow_invocations`; `durable.rs:1177`, `children.rs:275,355`, `durable.rs:906` | pass (source); execution gap |
| Slice: stale writers and retirement | Reused versions cannot target a replacement; old execution cannot become untouched backlog | Invocation-ID update predicate, exact completion comparisons, historical chapter evidence and current-only claim query | `durable.rs:1030`; `children.rs:241,268`; `chapters.rs:98,101,125`; authored store tests | pass (source); execution gap |
| Slice: populated conversion | Preserve prior bytes and reject ambiguous identity/ancestry atomically | Copies all retained columns; uniqueness, JSON-ID equality and Task FK; no compatibility view | `migrations.rs:4778` fixture compares bytes and uses savepoint rollback for duplicate IDs/dangling Tasks | gap: unrun; fixture uses direct SQL, not installed promotion |
| Done 1: ancestry and Session validators | Nullable parent equality, structural checks, stable Session/current Run, fenced repeated replacement | Task FK and invocation identity constraint only; Run/Session owner APIs absent | Schema and current Store APIs | gap |
| Done 2: one CLI reader | Launch/bind/rename; Session/Run/usage agree, including terminal Tasks | Manifest/subject reads remain; Session bind, rename and Task filter absent | `lf/mod.rs:721`, `lf/commands/runs.rs:51`, `ops/human_session.rs:269` | gap |
| Done 3: execution preservation | Shared taskless/Task storage and driver, captured nesting and exact review recovery | Task history retained; ordinary Flow file persistence and existing cursor model remain | `ops/flow_run.rs:108,136`; new and existing recovery tests unrun | gap |
| Done 4: Chapter operation | One repository boundary, frozen history, all-Wave transfer/retry | Activation still clears current only for one Wave | `store/sqlite/chapters.rs:39` | gap |
| Done 5: populated Home import | Preserve old SQL/files, idempotence, interrupted publication and cutover | Forward Task migration only; no offline Home importer or rehearsal | Branch source/migration diff | gap |
| Done 6: DTO/desktop | Stable Session pane across replacement/bind; typed ancestry and cached grouping | No Rust/Swift DTO, shared fixture or desktop change | Branch path inventory and current Session model | gap |
| Done 7: configured acceptance | Backed-up actual Homes, matching CLI/app, retained identities and latency measurements | No Home inventory, conversion, promotion, mounted pane or configured demo performed | No runtime acceptance receipt | gap |
| Done 8: deletion/consistency | Retired owners unreachable; docs match implementation; checks pass | Old Session/Run/taskless owners and Started writes remain; canonical docs are still the target spec | Negative searches and static results below | gap |

### Negative architectural proof and static checks

Searched current Rust callers, SQL writers, migrations, CLI and Session paths.
The retired `task_flow_positions` table has no current SQL reader or writer;
its remaining SQL occurrences are historical migration inputs and fixtures.
There is no compatibility view or duplicate Task write. Historical root
`step_index` and `iteration` still feed `decode_flow_position` and cannot be
dropped before conversion. Final claims remain historical evidence, not active
authority.

The broader forbidden outcomes remain reachable: `human_session::list` still
unions four sources; ordinary Flow `read`/`write` still use `position.json`;
Run listing still scans manifests and matches subjects through `WorkCatalog`;
provider identity, client receipts and Session resolution remain mutable files;
`begin_chapter_task` and the retained trigger still write Started. These are
existing incomplete cutover paths, not new adapters selected by this slice.
They must disappear through the approved replacement before publication.

| Check in this review | Result |
| --- | --- |
| Resource preflight, then `--recover` | FAIL as described above; no product test ran |
| `cargo fmt --all --check` | PASS |
| `uv run python scripts/check_migrations.py` | PASS: two ordered drafts, 52 shipped migrations unchanged |
| `uv run python scripts/check_architecture.py` | FAIL: SQLite coverage 30/31, missing `wave_chapters`; other seven inventories pass |
| Website environment: `uv run python ../scripts/render_architecture_html.py --check` | PASS |
| `git diff --check a48eeb6aed383123c51a6c6cc673591ae19b1417` | FAIL: earlier draft dependency-header trailing space and copied patch context lines |
| `git diff --check` for this review note | PASS |

Prior Clippy and documentation-test receipts retain their recorded scope on
unchanged executable/documentation bytes; they were not rerun here. The new
table removes the earlier architecture inventory gap for `task_flow_positions`;
it does not resolve Chapter ownership. Applied draft checksums and original
copied patch evidence were not rewritten for whitespace. A clean working-note
check cannot clear those branch-range failures.

### Next action and proof

Continue the approved coherent owner cutover, using
[concept-review](concept-review.md) for the two binding consequences. Preserve
current-Run-only binding as the recorded assumption, earlier Run attribution and
usage, and atomic taskless null-to-Task rejection under nullable equality.
Do not add another Session projection or restore the removed human column.

The first Session proof must replace its Run twice, retain identity/title/feedback
and both previous Runs, reject stale replacement and old-Run Ready/Complete,
then consume the saved feedback once. The same valid Session must remain
listable/openable beside an unrelated malformed invocation, with the latter's
bytes and identified recovery problem preserved. Taskless review recovery must
work after template removal through the same owner and exact settlement fences.

Once preflight permits, run the isolated focused commands from TESTING.md:

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib retaining_invocations_preserves_populated_execution_and_review_bytes
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
cargo test -p loopflow --lib stale_human_decisions_cannot_target_a_replacement_invocation
```

Repeat populated migration proofs after materialization in a disposable exact
source copy. Keep the real Home untouched during this proof. The complete
Chapter, Home import/cutover, DTO/desktop, configured acceptance, measurement,
and deletion obligations remain those in the design's eight Done When items.
This review chooses no navigation edge.

## Earlier review: projection-column removal

The assessment below records the earlier HEAD. Its two-table architecture gap
and 14.5 GiB resource reading are superseded by the current measurements above;
its Session availability and incomplete-cutover findings remain applicable.

2026-09-26 · LOO-298 · review-slice

Reviewed HEAD `6580b27cd3ccf5726dbb28c3efbdfee5b7629a76` against Task PR base
`a48eeb6aed383123c51a6c6cc673591ae19b1417`; the working tree was clean.
Scope: the complete branch diff, with executable changes limited to removing
four Task Flow projection columns and sharing their reader. Governing intent:
[amended design](data-model-one-table-per.md),
[completed review feedback](data-model-review-feedback.md), and
[Session ownership](session-runs-and-current-run.md). The later approval permits
taskless Invocations and makes Session the stable owner of Runs and a current
Run; the older Linear title and copied handoff do not override it.

**Disposition: not ready to publish.** This is a useful preparatory reduction,
with no new parallel owner, but its behavioral proof is still unexecuted and
the Task's owner cutover is absent. It is not an independently shippable slice
under the design. No PR mutation, Task completion, Home migration or Flow
navigation decision was made. This review changes only working notes.

## Findings

1. **Session availability now depends on every Task capture decoding.**
   `human_task_flow_positions` selects every position and calls
   `decode_flow_position` before testing `is_human`
   ([durable.rs](../rust/loopflow/src/store/sqlite/durable.rs#L392)). Previously
   SQL excluded autonomous rows. An invalid autonomous capture, cursor or claim
   now aborts discovery. `human_session::list` propagates that error before
   returning Ask, standalone Flow or interactive Sessions; exact Flow-session
   lookup also enumerates the same collection
   ([human_session.rs](../rust/loopflow/src/ops/human_session.rs#L269),
   [lookup](../rust/loopflow/src/ops/human_session.rs#L1162)). Thus an unrelated
   autonomous Task can make a valid review unavailable, not merely add a
   diagnostic to inventory. This is source-traced behavior, not an executed
   reproduction. Earlier notes identified the broader failure surface; this
   review follows its consequences through listing and opening a review.
   Resolve it in the Session owner cutover. Do not restore a stored human flag
   or silently discard corrupt execution evidence. Prove a valid unrelated
   Session remains listable/openable while the broken invocation remains a
   visible, identified recovery problem.
2. **The full target remains absent.** Current Session inventory is still the
   four-source union; ordinary Flows still read/write `position.json`; Run
   queries still scan manifests and match subjects. Bind, rename and
   `session list --task` are absent from this checkout's `SessionCommand`.
   Chapter activation remains Wave-scoped. These are existing remaining scope,
   not newly introduced adapters. They prevent treating documentation or this
   column deletion as the Task's acceptance result. In particular, the revised
   stable Session identity and current-Run transaction have no implementation
   or replacement-race proof yet.
3. **Proof remains unavailable, not green.** Resource preflight and safe recovery
   both fail on active `jack-heart/main-view-task`: 14.5 GiB / 12 GiB, with
   99.7 GiB free. [TESTING.md](../TESTING.md#bounded-and-honest) stops product
   tests under unresolved pressure. No foreign active build output was removed.
   The new migration and nested-review tests have prior compilation evidence,
   but neither that evidence nor this source review proves execution.

## Evidence matrix

`pass (source)` below establishes only the inspected shape. It does not mean
the corresponding behavior ran.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: remove four duplicates | Capture/cursor supply Flow, step, node and human policy | One shared SELECT/decoder; all three changed INSERT/UPDATE paths omit the four columns | Full Rust diff; search all `task_flow_positions` references; forward draft | pass (source) |
| Slice: preserve prior execution | Capture, cursor, feedback, versions, claims, failures and chapter start survive | DROP COLUMN only; historical root inputs and claim predicates retained | Populated preservation test at `migrations.rs:4677`; execution not run | gap |
| Slice: discover nested review and leave it | Selected captured XOR child supplies human policy; autonomous successor disappears from review discovery | New test checks discovery, feedback equality, exact read and stale write rejection | `review_discovery_follows_the_captured_nested_step`; execution not run | gap |
| Done 1: ancestry and Session validators | Nullable Task inheritance, mismatch rejection, stable Session/current Run, fenced replacement | Target records and validators not added | Current schema/types; no corresponding source change | gap |
| Done 2: one CLI reader | Launch, bind/rename, Session/Run/usage agree, including landed Tasks | File scans and selector matching remain; CLI verbs/filter absent | `lf/mod.rs:721`, `lf/commands/runs.rs:51`, `run_record.rs:426` | gap |
| Done 3: execution preservation | Common taskless/Task invocation owner; captured recovery and exact settlement | Existing Task SQL and ordinary file adapters remain; fencing preserved in changed SQL | `ops/flow_run.rs:34,108,136`, `store/sqlite/durable.rs:1120`; existing tests not rerun | gap |
| Done 4: Chapter operation | One repository boundary, all Waves, frozen history | `save_chapter` still clears current by Wave | `store/sqlite/chapters.rs:33` | gap |
| Done 5: populated import | Import all old owners, idempotence, interruption and preservation | Only projection-column migration; no Home importer | Complete migration diff | gap |
| Done 6: DTO/desktop | Stable Session pane, current Run/history, cached typed grouping | No DTO, fixture or Swift change | Complete branch diff | gap |
| Done 7: configured acceptance | Backed-up Homes, matching CLI/app, real demo and measured reads | No import or installed candidate acceptance | No live mutation or demonstration attempted | gap |
| Done 8: deletion and consistency | Retired owners unreachable; docs match final runtime; checks pass | Four-source union, subjects, mutable sidecars, file Flow owner and Started writer remain | Negative searches below; architecture checker fails two owners | gap |

## Negative architectural proof

The searched current paths still include:

- `ops/human_session.rs:269`: Task reviews + Ask files + standalone Flows +
  interactive Runs; `list_ask_sessions` at 1131 reads JSON records.
- `ops/flow_run.rs:108,136`: reads and atomically replaces the ordinary Flow
  file; this still supports optional Task selection.
- `lf/commands/run.rs:740`: converts selector strings to `SubjectAttribution`
  for capture; `lf/commands/runs.rs:57` scans then filters through `WorkCatalog`.
- `run_record.rs:640,774,790,912,934`: provider identity, client attachments and
  Session resolution remain mutable filesystem state.
- `store/sqlite/chapters.rs:78`: writes Started history; the current chapter
  trigger also remains. Their retirement safety consumer is still live.

No current SQL reference to the four removed columns was found outside
historical migrations and migration fixtures. `step_index` and `iteration`
still feed the historical flat-progress decoder, so retaining them is justified.
No new Legacy/New dispatch, dual write or generic abstraction was introduced.
The cut advances the design locally without establishing its deletion boundary.

## Checks and demonstration boundary

| Command / evidence | Result in this review |
| --- | --- |
| `uv run python scripts/resource_envelope.py` | FAIL: active `main-view-task` build over budget |
| `uv run python scripts/resource_envelope.py --recover` | Same FAIL; active build preserved |
| `cargo fmt --all --check` | PASS |
| `uv run python scripts/check_migrations.py` | PASS: one draft; 52 shipped migrations unchanged |
| `uv run python scripts/check_architecture.py` | FAIL: SQLite map 29/31, missing `task_flow_positions`, `wave_chapters`; other seven inventories pass |
| `uv run python ../scripts/render_architecture_html.py --check` from `website/` | PASS |
| `git diff --check a48eeb6aed383123c51a6c6cc673591ae19b1417` | FAIL: blank draft dependency header has trailing space; inherited patch has space-only context lines |

The first HTML check was invoked from the root Python environment and failed
to import `fasthtml`; the prescribed website environment passed. That setup
failure is not an HTML regression. Earlier Clippy and documentation-test passes
apply to unchanged executable/documentation bytes; they were not rerun here.

The branch-range whitespace failure is narrower than a product defect but
means the earlier generic whitespace-pass wording is not a clean whole-branch
receipt. The copied patch retains its original context bytes. The already
committed migration was not rewritten for cosmetic whitespace: installed draft
checksums can make that a schema-history change.

The most important available local demonstration would be the populated SQLite
upgrade followed by nested review discovery. The resource boundary prevented
running it. An installed CLI would exercise different schema/code and cannot
substitute for that proof. No live, simulated-provider, or desktop behavioral
pass is claimed.

## Next action and proof

Once preflight passes, execute the two owed focused commands in the isolated
environment described in TESTING.md:

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
```

Also prove the populated migration on materialized drafts in a disposable
source copy, never by materializing this worker's checkout. The existing
regression checks retained bytes and recovered capture/cursor; full import
will additionally need typed, valid historical claims and end-to-end recovery.

Continue slices 2–4 as one owner cutover: typed Invocation/Run/Session storage,
shared taskless and Task execution persistence, exact current-Run replacement,
all writers/readers, then DTO/desktop consumers. Include the unrelated-invalid-
invocation counterexample above. Preserve the current-Run-only bind assumption
without rewriting earlier Runs. Follow with repository Chapter conversion,
Home rehearsal and configured acceptance, then deletion research. Keep the
present-tense documentation spec; make the code match it before publication.

No bounded executable fix was selected in this review: restoring a projection
or skipping corrupt records would undermine the chosen owner model, while the
required replacement spans the unimplemented architectural slice. The following
decision step owns navigation using this evidence.
