# LOO-938 — Keep the plan on the hub Machine's server

## Direction (2026-11-18)

Jack Heart: "the plan syncing was written in a pre-machine world"; "I think
i decided towards ahving one plan owner machine server now"; "i would prefer
to just turn this task into the proper design rather than continue to churn".

- **The hub owns the plan.** The always-on hub Machine's server holds the plan
  for every repository without Linear or Asana; other Machines and the phone
  read and write it through the hub. Amends LOO-861's "reads no Work of the
  Machine it runs on" (comment posted on LOO-861, 2026-11-18).
- **PR #1971 is closed, no interim fix.** Its head is kept at
  `refs/backup/loo-938-pr1971` (18a15eb3ef).

History: `scratch/plan-ownership-history.md`.

## The design in one paragraph

The hub is a third tracker. `Tracker { Linear, Asana, Hub }`: a Machine that
is not the hub queues every local planning change as a `tracker_actions` row,
exactly as a Linear repository does, and the drain sends it to the hub's
server; inbound changes arrive from the hub's `plan.changes` feed through the
same `planning::rules::decide` that Linear events use. On the hub Machine
itself the store *is* the plan and nothing is queued. The Git planning ref,
the peer journal, field causality, destinations and the per-Session exchange
are deleted. LOO-925's queue, its open/lost view and Desktop's Keep / Use mine
become the one way every tracker shows "where does my change stand".

This is what the branch name already said: send every planning change through
one queue.

## Findings that shaped it

- **The seam exists.** `ops/tracker.rs` `Tracker` has `idempotent`, `blocked`,
  `send`, `look`, `changes`, `fetch_issue`, plus a `#[cfg(test)] Fake`. A
  `Hub(HubClient)` variant slots in; `queued_tracker` gains `Hub`.
- **The carrier exists.** #1956 (LOO-926) sends typed requests to another
  Machine's server with `ssh <machine> lf server connect`, protocol revision 8,
  version-skew errors naming the fix. The hub client uses that carrier now;
  LOO-861 swaps it for the hub's persistent outbound connection later without
  changing the operations. **So LOO-938 does not wait for LOO-861**, which is
  still unstarted.
- **Offline needs no causality.** Linear repositories already answer the laptop
  on a plane: save locally, queue, show "3 open", send on reconnect, show a
  losing value as lost. The hub inherits all of it. The single authority
  replaces field causality with a per-field sequence check at the hub.
- **"Git-planned" is the default, not a choice.** `TrackerConfig::Git` is what
  a repository with no `tracker:` key gets (`config.rs:315`, `GitPlan::Shared`
  default). etude has no `tracker:` key. So the new rule needs no repository
  key: no Linear/Asana → the plan is on this Machine's hub, or local when it
  has none (LOO-406's store-is-home, unchanged for a single Machine).
- **etude is the only live user.** `lf repo tracker` in `~/src/etude`:
  `shared/main` 397 records, publication confirmed; `users/9a0c1265…` 22
  records, "local changes pending" and "Git acquisition failed". Git Tasks
  carry no identifiers (`lf task list` shows `identifier: null`); they are
  addressed by UUID, which is also what a hub `create` uses as tracker
  identity, so rows bind by id.
- **mini was unreachable during this read** (`ssh mini`: hostname does not
  resolve; `100.96.227.95:22` timed out). Whether mini has an etude checkout
  and a current `lf` is unverified; slice 3's cutover checks it.

## Shape

### Which Machine is the hub

A Machine stores one hub: `lf machine hub mini` names a saved Machine
(`lf machine add` already saves them); `lf machine hub --clear` removes it;
`lf machine list` marks it. LOO-861 later owns this same setting as its "one
hub address". A Machine whose own `machine_id` equals its hub's id, or with no
hub, is authoritative for its own plan. *Made for review:* a Machine-level
setting rather than a repository `tracker: hub` key, because who the hub is
depends on the person's Machines, not the repository; passed over: a
`tracker: { hub: <machine> }` repository key.

### Hub operations (server protocol, new revision)

- `plan.apply { repository, actions: [TrackerAction] }` → per action:
  `Applied { seq }` or `Conflicted { hub_value, seq }`. Batched: one request
  carries a drain pass's actions for the repository, so a pass costs one SSH
  connection, not twenty.
- `plan.changes { repository, since: Option<seq> }` → owners (Projects, Tasks
  with comments) changed after `since`, in store words, and the new `seq`.
  `since: None` is the full plan.

The hub records, for every planning write in a repository it is authoritative
for, `plan_field_changes(repo, owner_id, field, seq, action_id NULL)`: one row
per (owner, field), `seq` a store-wide increasing integer set in the writer's
transaction (`sqlite/planning_write.rs`, which already derives the conflict
field from the `WorkValue`). Index `(repo, seq)`. Rule at apply: an action
carries `base` = the seq its Machine had imported when it saved; if the
field's row has `seq > base` and a different `action_id`, it is conflicted,
else applied. Same `action_id` → already applied, answer `Applied` again
(lost-reply repeat is safe, so `Hub` is `idempotent` for every kind).

Comments and ownership entries (marked comments, LOO-523) travel as `comment`
actions; ownership semantics are unchanged.

### Non-hub Machine

- Local writes: unchanged rows, `WriteOrigin::Local` queues `tracker_actions`
  with `tracker = hub`. Binding: `tasks.external_issue_id` = the Task's own
  UUID; Projects likewise.
- Drain: `ops/tracker/drain.rs`, existing limits (20 requests / 5 s pass),
  Hub sends the whole batch in one `plan.apply`.
- Inbound: `refresh` at the existing boundaries (before Graph steps, after
  agent turns, when commands show planning; Desktop / `--subscribe` clock)
  calls `plan.changes` with the watermark kept in `linear_team_checks`-style
  per-scope row (scope = repository plan id). Applying is `WriteOrigin`
  import: no echo actions.
- Hub unreachable: actions stay queued; `lf repo tracker` says "hub mini
  unreachable since 14:02 · 3 open"; reads show the local store. Never an
  empty plan, never a fallback to Git.
- Version skew: #1956's errors, unchanged ("mini runs lf 0.15.2 … update it
  with `ssh -- 'mini' 'lf install'`").

### Hub Machine

Writes go straight to its store and bump `plan_field_changes`; nothing is
queued. Its server answers `plan.apply` by running the same typed writer with
the sender as `WriteOrigin::Local` minus queueing. It needs the repository
registered (`work_route::repository_path`); a missing one answers with the
command that adds it.

## User-visible outcome

On etude, from the laptop: `lf task create 'x'` returns at once; `lf repo
tracker` shows `1 open` then `0 open` after the save's drain; on mini, `lf
task list` shows the Task. Laptop offline: create and edit work, show open,
and arrive when the laptop reconnects. Two Machines editing one Task's title
while one is offline: the late one shows `1 lost` with both values, and Keep /
Use mine answers it, in CLI and Desktop, with the code Linear already uses.
Desktop loses the Git destinations list; its tracker line reads the same as a
Linear repository's.

## Acceptance check at gate

Headless, two homes on one Mac with #1956's stand-in `ssh`
(`.lf/qa/machine-moves.md`), home B's hub = home A:

```bash
B: lf task create 'hub smoke' --json          # receipt; B: lf repo tracker → "1 open"
B: lf task sync <id>                           # → "0 open"
A: lf task show <id> --json                    # title "hub smoke"
A: lf task edit <id> --title one ; stop A's server
B: lf task edit <id> --title two               # queued, "hub … unreachable · 1 open"
start A's server; B: lf task sync <id>         # → "1 lost" (local two, hub one)
B: lf repo tracker answer <action> --use-mine  # A: title "two"
```

Plus a Rust integration test for the same sequence through `Tracker::Hub`
with the carrier mocked at the `lf server connect` pipe.

## Affected surfaces

- `ops/tracker.rs`, `ops/tracker/drain.rs`, `planning::rules::decide` (a Hub
  event is a seq, not a Linear revision).
- `sqlite/planning_write.rs` (seq rows), new migration (`plan_field_changes`,
  `-- leaving:` for every `git_tracker_*` table and the `planning_peer_*` /
  `planning_destinations` copies, `planning_user` if nothing else reads it).
- Server protocol + `tests/fixtures/dto/server_schema.json`, Swift
  `ServerProtocol`, Pi extension, `server_connection.js` (revision bump only).
- `lf repo tracker` / `TrackerActionStatus`: hub reachability line replaces
  destination receipts; `lf repo tracker journal` goes.
- `lf machine hub`, `lf machine list`.
- `.lf/config.yaml`: `tracker: { git: … }` is a shared file the released `lf`
  reads; the branch parses it and treats it as "no tracker", marked with a
  removal Task filed in this change (AGENTS.md shared-file rule).
- Docs: `docs/dev/planning.md` (Git sections), `docs/dev/machines.md`,
  `docs/glossary.md` ("destination"), `rust/loopflow/src/server/README.md`,
  `rust/loopflow-storage/SCHEMA.md`.

## What goes

- `planning_git.rs` (437 lines); Git half of `ops/planning_sync.rs`
  (`fetch_repository`, `publish_repository*`, `exchange_destination`,
  `fetch_destination`, `publish_destination`, `wait_for_publication`,
  `git_destinations`, `git_journal`, `adopt_personal_key`,
  `configured_destination`, `defer_exchange`, `PlanningSync` and its three
  starters: `harness/native_session.rs:526`, `agent/mod.rs:1019`,
  `transport/hosted.rs:101,904`).
- `loopflow-storage`: `git_tracker.rs`, `sqlite/git_tracker.rs` (1116),
  journal parts of `planning_sync.rs` / `sqlite/planning_sync.rs`, causal
  parents in `WorkEdit` mutations, `execution/store/git_tracker.rs`.
- `TrackerConfig::Git`, `GitPlan` (`Shared`/`Personal`) after the marked
  fallback.
- Tests: `tests/planning_git_tests.rs`, `planning_reconnect_tests.rs`,
  `tests/e2e/planning_reconnect.py`, Git parts of `planning_setup_tests.rs`,
  `planning_foreground_tests.rs`, `store/planning_sync_tests.rs`,
  `planning_exchange_lifecycle_tests.rs`.
- Swift: `GitDestination.swift`, `GitDestinationsView.swift`,
  `PeerPlanning.swift` and their DTO fixture `git_destinations.json`.
- PR #1971 and LOO-899 / LOO-868 patches go with the code they patched.

Survives: every Task/Project ID, comment and ownership entry; the
`tracker_actions` queue; Machine-local Sessions, Processes, checkouts,
reservations (Infrastructure Wave memory "Plan ownership").

## Cutover for etude

Not code (no cross-repo migrations). Before installing: with the released
`lf`, `lf task sync` on every Machine until `lf repo tracker` shows both refs
confirmed, then on mini. Install. `lf machine hub mini` on the laptop; the
first `plan.changes(None)` binds rows by UUID. Rows on the laptop the hub
lacks (the 22 personal records if mini never fetched them) get a `create`
action on first contact: a non-hub Machine queues `create` for every
unbound local Project/Task in a hub repository once, when its hub is set.
The ref stays on origin, inert; deleting it is one hand-run
`git push origin :refs/loopflow/planning/…`.

## Exclusions

- Linear/Asana drains stay per Machine (LOO-925). Moving them to the hub is a
  later question (`scratch/questions.md`).
- The persistent hub connection, presence, phone pairing: LOO-861.
- Task identifiers for hub repositories (Tasks stay UUID-addressed).
- Several people on one hub: the apply rule is per field, not per person; no
  admission beyond SSH-as-owner, as #1956.

## Slices

1. **Hub as a tracker, one field.** `Tracker::Hub` with `send` over #1956's
   carrier, `plan.apply` on the server for `field` actions,
   `plan_field_changes` in the writer, `plan.changes`, and `lf machine hub`.
   Shows: a title edited on home B appears on home A, and a concurrent edit
   with A's server stopped comes back `lost`. This is the slice that can
   prove the plan wrong: if `decide` and the action baselines don't fit a seq
   authority without a parallel rule set, the "third tracker" framing fails
   and the plan changes before anything is deleted.
2. **Every kind.** `create` (first-contact binding by UUID), `state`,
   `order`, `comment` (ownership entries), `delete`, `link`; the full
   acceptance check runs.
3. **Delete Git.** Everything in "What goes", the migration's `-- leaving:`
   marks, docs, Desktop's tracker line. Run the etude cutover against mini.

Checks: focused Rust tests per slice; `lf repo tracker` on etude before and
after cutover (gate, by hand); Desktop view tests for the tracker line (CI).
