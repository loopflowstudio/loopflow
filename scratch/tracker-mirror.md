# LOO-925: the tracker mirrors the store

## Decision: any tracker — Linear and Asana bridges, Notion deleted

The store is the plan. A tracker is a bridge that carries the store's changes
out and its coworkers' changes in. Linear and Asana are the two bridges; Notion
is deleted. Git planning is not a bridge: it exchanges the store's own WorkEdit
journal between peers and keeps working as it does.

Basis, all recoverable only from here:

- LOO-691's unmerged branch `loo-691-combined` (last commit e8ffd80ca0,
  2026-11-04, "checkpoint the interrupted parity pass before splitting the
  diff") records in `scratch/design.md`: "Jack Heart, 2026-11-02: drop Notion
  in this diff; regressions on Linear, Asana and Git planning; feature and
  architectural parity between the three (one interface at one layer, no
  Linear escape hatch or Linear-named shared types)." That is the design
  doc's wording, not a transcript. The Notion delete (cd7afe0127) never
  reached main; regressions shipped instead as GitHub issues (#1780).
- Asana was rebuilt 2026-10-22 (#1607) after an earlier delete (c113ef04b4,
  2026-07-02, "asana-only: drop Linear/Notion"). The README promises "whatever
  … tracker you already use".
- The neutral queue is the Outcome under either choice, so "any tracker" costs
  only keeping Asana (~1.3k lines) on the bridge interface. Linear-only would
  delete Asana a third time.
- Run observation, 2026-11-16, `~/.lf/loopflow.db`: every `pm_items`
  (1,246) and `pm_projects` (57) row is `linear`; all 118 `.lf/config.yaml`
  planning blocks under `~/src` are Linear. No store on this Machine has Asana
  data, so making `provider` truthful needs no data repair.

Overturnable at review: `scratch/questions.md`.

## Demo

```bash
export LF_HOME=$(mktemp -d)        # branch lf, Linear stand-in (test fixture) or a sandbox Team
lf task create --title "Mirror demo" --json
lf task edit <id> --title "Mirror demo, renamed"
lf steer <id> "first comment"
lf task complete <id>
lf repo plan --json                # one queue: 4 effects, seq order, kind, phase, provider, receipt
lf task show <id> --json | jq .sync
```

Switch `.lf/config.yaml` to `provider: asana` against the Asana stand-in and
run the same five commands: the same queue rows, `provider: asana`, and
`SELECT provider FROM pm_items` answers `asana`. Pull the network: every
command saves, `lf repo plan` lists the effects as pending, and the next
connected command drains at most one bounded pass.

## Current architecture (at 470da475a1)

- **Journal already neutral.** `sqlite/planning_write.rs` is the one typed
  writer; every planning write is a `WorkEdit { target, WorkValue }` captured
  in `planning_peer_changes` (42,315 rows in the live store) with clock and
  parents. Git exchange ships these. Trackers ignore them.
- **Seven outbound receipts, four drains.** Receipts: `task_creation_intents`
  and `project_transitions` (`export_json` holds Linear GraphQL input built in
  `loopflow-storage/src/sqlite/planning_export.rs:130`, with
  `linear_records::render_project_content`), `task_changes` and
  `project_changes` (already `field` + `value_json` WorkValue +
  `base_json`/`conflict_json`/`attempted`/`acknowledged`; deletion is
  `task_changes.field='deleted'`, order is `project_changes.field='task_order'`
  + `order_effects_json`), `task_state_deliveries`, `task_comment_deliveries`.
  Drains: `ops/planning_export.rs::sync_repository_exports`,
  `ops/planning_delivery.rs::sync_repository_fields` (+ `planning_order.rs`),
  `ops/linear_observe.rs::sync_repository_deliveries` (state, comments),
  called from `planning_sync::sync_after_save` and `linear_observe::sync_scope`.
  Each joins over every pending owner with its own 5 s deadline.
- **Live store backlog, 2026-11-16:** `task_changes` 503 acknowledged, 167
  attempted, 86 pending; `task_state_deliveries` 1,064 rows; 2,656 comment
  deliveries. 167 attempted-and-never-settled field edits is the lost-reply
  rule failing quietly.
- **Execution builds Linear input too.** `planning_delivery.rs::task_input`
  emits `assigneeId`/`projectId`; Asana's `asana/mod.rs::task_fields` (664)
  translates Linear keys back. `PlanningClient` (`ops/planning_client.rs`) is
  a 3-arm enum of passthroughs returning `LinearIssue`/`LinearResult`;
  `PlanningClient::linear()` is the escape hatch (order delivery, Project
  edits).
- **Stall fix to keep** (#1829, b9339916f1): no freshness check inside a
  write; one `ListTeams` per process (`VALIDATED_TEAM_CLAIMS`); an uncertain
  creation is looked for once per `max(600 s, delta_fresh_secs())` via
  `export_observed_at`; `lf task sync ISSUE` looks now.
- **Revision triggers.** Every receipt table has
  `store_revision_<table>_{insert,update,delete}` bumping domain `planning`;
  `PlanningSyncStatus` (`loopflow-storage/src/planning_sync.rs:48`) feeds
  Task/Wave frames and Desktop.
- **Mixed versions.** `lf install` migrates while older lf processes keep
  running on the same store (`MIGRATIONS.md`, "Who applies what"); there is no
  reader floor any more. An older lf drains the old tables for the window.
- **Docs disagree with code.** `planning.md:88` "there is no generic planning
  facade"; `planning_client.rs` is one. `docs/work.md:228` documents Notion.

Prior attempt to mine before writing code: `loo-691-combined`
(f26a384079 shared records into `crate::planning`; 03b9d4311b Asana Project
status/order/assignee and `PlanningClient::linear` deleted; cd7afe0127 Notion
delete incl. OAuth provider and docs; `scratch/parity.md` capability ×
provider table). It is +4.3k/−4.3k over 66 files against an old base and was
never split; reuse its ideas and inventories, not its diff.

## Target architecture

**One queue, in the store's words.** New table `tracker_effects`:

| column | meaning |
|---|---|
| `seq` INTEGER PK AUTOINCREMENT | global send order |
| `id` TEXT UNIQUE | effect identity; the journal mutation id when there is one; the provider idempotency key where the provider accepts one (Linear create, comment) |
| `wave_id` / `project_id` / `task_id` / `comment_id` | exactly one owner (CHECK); a Wave's name and summary are its Linear Initiative or Asana team fields |
| `kind` | `create` · `field` · `order` · `state` · `comment` · `delete` · `link` (Initiative link or PR link) |
| `value_json` | the `WorkValue` (or, for `create`, the creation baseline `Vec<WorkValue>`); never provider input |
| `base_json`, `conflict_json` | provider baseline at enqueue; losing value after a conflict |
| `phase` | `pending` · `attempted` · `acknowledged` · `retired` |
| `provider` | the bridge that was asked, set at attempt |
| `remote_id`, `acknowledged_revision` | what the provider answered |
| `error`, `attempted_at`, `looked_at` | delivery evidence |

Partial index `(phase, seq) WHERE phase IN ('pending','attempted')`; revision
triggers on domain `planning`, with the update trigger counting `looked_at`
only on NULL ↔ value.

**Every write has a receipt, structurally.** `planning_write::write` with
`WriteOrigin::Local` inserts the effect row in the same transaction for every
`WorkValue` a bridge carries; creation (`planning_write::create`) inserts the
`create` row. Provider calls for planning facts happen only in the drain.
GOAL.md's name and summary reach the Initiative as Wave `field` effects
(today `refresh_async`'s apply branch, `ops/linear.rs:1749`, sends
`update_wave` with no receipt, and only a test reaches it). Initiative rename
and archive keep their `wave_lifecycle_operations` receipts.

**Bridges translate at the edge.** `ops/tracker.rs`:
`enum Tracker { Linear(LinearClient), Asana(AsanaClient) }` replaces
`PlanningClient`, built from `PlanningConfig`. Its methods take and return
store words: `send(&Effect) -> Sent`, `look(&Effect) -> Look` (readback after a
lost reply), `item(id) -> planning::Observation`, `changes(since)`. Linear
GraphQL lives in `linear/effects.rs`; Asana REST in `asana/effects.rs`. The
records a bridge answers (`Item`, `Observation`, `Comment`, `Project`,
`StateHistory`, `PlanningResponse`) move from `linear_records` to
`loopflow_storage::planning` (as f26a384079 did); `Linear*` types stay inside
`linear/`. No `linear()` escape hatch.

**Per-kind rules as functions, one lost-reply rule.**
`ops/tracker/rules.rs`: `fn settle(effect, observation) -> Settlement { Send,
Acknowledged(revision), Adopt { losing }, Hold }`, one function per kind,
carrying today's rules from `planning.md:204-272` (field baseline conflict
adopts the provider and keeps the losing value; order needs complete-list
evidence; deletion settles on trash or delete-ack, a newer active revision
retires it; state compares revision/state/completion; comment matches id or,
on Asana, visible text). Lost reply, for every kind: an `attempted` effect is
looked for before anything else on its owner is sent, at most once per
`max(600 s, delta_fresh_secs())` (`looked_at`, generalizing
`export_observed_at`); idempotent sends (Linear create and comment with our
id) re-send instead of looking.

**Bounded drain.** `ops/tracker/drain.rs::drain(store, repo, Bound)` replaces
the four drains. One pass reads unsettled effects in `seq` order; an owner
whose head is `attempted` and inside its window is skipped (head-of-line per
owner, not global); the pass stops after 20 provider requests or 5 s,
whichever first; owners run with concurrency 4. The per-object effect lock
file path (`planning_delivery::lock_fields`) is kept so an older lf and this
one never send for the same object at once. Each send keeps its own 5 s
deadline: a shared deadline was removed on purpose because a slow first export
starved every later Task (`planning_sync.rs:81`, `linear_observe.rs:336`), so
the bound counts requests, and `sync_after_save` drains the just-saved owner
before the bounded remainder. Requests go through the existing
`linear/cooldown.rs::Budget` (creations stay `urgent`). `lf task sync <id>`
drains one owner, ignoring windows. A pass with nothing unsettled sends zero
requests.

**Truthful names.** `provider` columns get the configured bridge's name at
every writer: 14 SQL literals and 17 bound `"linear"` parameters in
`loopflow-storage` (mostly `linear_acceptance.rs`), several reached by Asana
through `planning_delivery::ingest_task` and `check_scope`. The acceptance
functions take the provider as an argument. `ops/linear_observe.rs` becomes
`ops/tracker_observe.rs` (it is the generic delta and comment path for every
provider, with Linear branches at :65, :102, :459). `tests/linear/` (Graph,
PR, status, subscribe, Task launch, Wave resolution: almost none of it Linear)
becomes `tests/work/`; `tests/README.md:400` names a missing
`adoption_tests`.
`planning.md`, `docs/work.md`, README say Linear or Asana.

### Migration (one, additive): `tracker_effects`

`uv run python scripts/new_migration.py tracker_effects`. Creates the table,
index and triggers; copies every unsettled row from the seven receipts into
it, translating `export_json.input` back to the creation baseline already
stored beside it (`export_json.baseline`); then marks the copied old rows
settled (`acknowledged=1` / settled flags) so an older lf on the same store
does not send them a second time. New code never reads or writes the old
tables. Upgrade fixture: seed a store at origin/main's schema under
`rust/loopflow-storage/tests/fixtures/migrations/` with one unsettled row of
each kind, migrate, assert seven effects and zero sendable old rows.

Window risk, accepted: an older lf process still running after install
writes its own new edits to the old tables and delivers them itself; if it
exits first they stay unsent. Its inbound acquisition, seeing no old pending
receipt, can adopt a coworker's newer value over a local edit that exists
only in `tracker_effects`; the new drain then sees the baseline moved and the
field rule adopts the provider, keeping the losing value. Same outcome as a
conflict today.

## Delete — do not maintain

Each line deletes its exclusive tests and fixtures.

- **Notion:** `notion/` (`mod.rs`, `blocks.rs`), `ops/linear/notion_planning_tests.rs`,
  `PlanningConfig::Notion`, `PlanningClient::Notion`, the Notion OAuth provider
  and `account connect notion`, docs/website/glossary mentions. Mine cd7afe0127.
- **The generic facade:** `ops/planning_client.rs` (→ `ops/tracker.rs`),
  `PlanningClient::linear()`, `planning_client::is_linear` and its callers,
  `asana::task_fields`.
- **Store-built GraphQL:** the `json!` inputs in
  `sqlite/planning_export.rs::prepare_planning_export`, `export_json.input`,
  `linear_records::render_project_content` in storage (moves to `linear/`).
- **Old drains and receipt writers:** `ops/planning_export.rs`,
  `ops/planning_delivery.rs`, `ops/planning_order.rs` delivery,
  `linear_observe::{sync_repository_deliveries, deliver_task_state,
  sync_task_comments}`; storage receipt writers in `planning_changes.rs`,
  `task_state_delivery.rs`, `planning_order.rs`, the delivery half of
  `task_comments.rs`, and `task_creation_intents`/`project_transitions` export
  columns' writers. Behavior that survives: every rule above, the #1829 window,
  `lf task sync`, `--retry-updates`, `PlanningSyncStatus` fields.
- **Chapter mechanisms:** `ChapterSweep`, `chapter_sweep_candidates`,
  `require_outside_current_chapter` (`ops/linear.rs:2108-2200`) and the `lf
  task sweep` command that runs them (`lf/mod.rs:2482`,
  `ops/task/lifecycle.rs:294`); `ops/linear/adoption.rs` and
  `sqlite/project_adoption.rs`, whose read half runs on every `lf wave sync`
  (`refresh_inventory` → `checked_projects_with_store`, `linear.rs:1978`) and
  whose apply half is reachable only from a test. Rename live names and output:
  `chapter_metric_portfolio`, `validate_chapter_targets`,
  `metric::ChapterUnavailable`, "unset for this chapter" (`waves.rs:2747/2750`),
  "Chapter plan:" (`ops/run.rs:39`), `wave_lifecycle.rs` removal counts, CLI
  help in `lf/mod.rs`. Kept: builtin skills named `*-chapter` (AGENTS.md: a
  Chapter is the In Progress Project's shared name), migrations, and the
  `~/.lf/chapter-locks/` path, which an older lf locks on the same inode
  (`planning_sync.rs:595`); the contract Task renames it.
- **`pm_issue_changes`:** the brief says it has no production writer; the
  sweep found one. `observe_pm_issue_change` (`sqlite/linear_observations.rs:10`)
  records a provider removal from `linear_observe.rs:53`, and five readers
  (`linear_issue_evidence` ×2, `task_acquisitions_in`, `put_item`,
  `project_accepted_planning`) use it as removal/invalidation evidence. Slice 1
  first writes `a_tracker_removal_withdraws_new_launches`; if
  `tasks.planning_deleted_at` and `task_deletions` already carry that fact the
  writer and readers go; otherwise removal becomes a column on `pm_items`
  in the migration and the table goes in the contract.
- **Receipt-skipping writers:** the non-plan branch of `refresh_async`
  (`ops/linear.rs:1745-1762`, reachable only from
  `task_planning_tests.rs:4131`; `lf machine doctor --planning` runs plan
  mode); PR links (`linear.rs:766-798` save ids after the call;
  `ops/task.rs:2481` reposts an id-less Asana comment on every publish) become
  `link` effects. Out of scope, explicit admin commands with pre-validation:
  `connect_async::create_wave` (`linear.rs:650`) and reteam
  (`linear.rs:1378-1452`).

Table drops (`task_changes`, `project_changes`, `task_state_deliveries`,
`task_comment_deliveries`, export columns, `pm_issue_changes`) and table
renames (`task_linear_observations`, `linear_team_checks`) are a contract
migration ≥ 30 days after this merges: a follow-up Task filed with this PR.

## Forbidden outcomes

- New code reading or writing an old receipt table (dual read/write).
- A `Linear*` type, GraphQL key or `match` on provider outside `linear/`,
  `asana/` and `ops/tracker.rs`.
- A drain that loops over every unsettled owner without a request bound.
- Notion code left behind "for later"; a `PlanningClient` alias.

## Exclusions

- Wire DTO field `linear_id` (in ~10 `tests/fixtures/dto/*.json`, Swift
  mirrors) and `LinearIssueId`/`LinearProjectId` newtypes: renaming crosses
  Rust/Swift and every fixture; follow-up Task.
- Inbound acquisition protocol (deltas, watermarks, team checks) beyond
  writing the real `provider`.
- Asana feature gaps (reteam, Initiative connect) that `parity.md` marks n/m.
- Git planning exchange.

## Slices

Deepest deletions first, then the queue, then the bridges. Commit at each.

1. **Done (slice 1 commit) — deletions.** `pm_issue_changes` deferred to slice 3
   with the inbound removal path. Notion, Chapter mechanisms and `lf task
   sweep`, adoption, the `refresh_async` apply branch, the `pm_issue_changes`
   decision above. Check: `cargo build -p loopflow`; `cargo test -p
   loopflow-execution --lib asana_planning_tests`; `grep -rn -i notion rust/ --include=*.rs`
   empty outside migrations.
2. **Done (slice 2 commit).** Shared records are `loopflow_storage::planning::{TrackerItem, TrackerObservation, TrackerComment, TrackerProject, StateHistory}`; `PlanningResponse` dies with the old drains in slice 3.
3. Migration + writer enqueues + `rules.rs` + bounded `drain.rs`; delete the
   old drains and storage GraphQL. Focused tests per kind on the Linear fake:
   lost reply, conflict adopt, deletion retire, order evidence; plus
   `a_pass_with_nothing_unsettled_sends_no_request` and
   `a_pass_sends_at_most_twenty_requests` (100 pending effects).
4. `Tracker` enum and the two `effects.rs`; delete `PlanningClient`; Asana
   tests move onto the queue.
5. Truthful `provider`; renames (`tracker_observe.rs`, `tests/linear/`).
6. `planning.md`, `docs/work.md`, README; file the contract and DTO Tasks.

## Done when

- Creating, editing, steering, completing, reordering and deleting a Task in a
  Linear- or Asana-configured repository each leave one `tracker_effects` row
  that `lf repo plan --json` shows and the next pass settles.
- `grep -rn 'teamId\|stateId\|assigneeId' rust/loopflow-storage/src` is empty.
- `grep -rln -i notion rust/*/src --include=*.rs | grep -v migrations` is empty;
  `grep -rn -i chapter rust/*/src --include=*.rs | grep -v migrations` lists
  no live behavior.
- A pass with 100 unsettled effects sends ≤ 20 requests; a pass with none sends 0.
- `planning.md` describes one queue, two bridges and the lost-reply rule.

## Gate checks (headless)

- `uv run python scripts/check_migrations.py` → passes (additive).
- `cargo test -p loopflow-storage --lib tracker_effects` → upgrade fixture passes.
- `cargo test -p loopflow-execution --lib ops::tracker::` → per-kind and bound tests pass.
- `cargo test -p loopflow-execution --lib asana_planning_tests` → passes on the queue.
- `EXPLAIN QUERY PLAN` of the drain's select on a 10k-Task store → uses the partial index.
- CI runs the suites.

## Wild success / failure

Success: someone on Asana gets the same `lf repo plan` and conflict
story as Linear, and a third bridge (GitHub Issues, Jira) is one `effects.rs`
plus a stand-in. 167 forever-attempted edits stop existing because the
lost-reply rule is one function with one test.

Failure: the change grows like LOO-691 (66 files, never split) and stalls;
or the migration's copy double-sends through an older lf. Guards: slices land
as commits that build; old rows are marked settled in the same migration.


## User docs (written ahead of code)

`docs/work.md` ("Linear or Asana", Reference "Sync"), `docs/glossary.md`,
`docs/config.md`, `docs/dev/data.md`, `README.md`. The queue surfaces in the
existing `lf repo plan` (there is no `lf planning` command); its text output
is documented as one line per unsettled effect: owner, kind, `pending` or
"sent, waiting for <tracker> to confirm". `lf task sync <id>` drains that
owner now. Still to update with the code: `docs/dev/planning.md`,
`docs/dev/reference.md:89`, `rust/loopflow-execution/src/ops/README.md:10`.
