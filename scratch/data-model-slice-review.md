# Data model storage slice review

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
