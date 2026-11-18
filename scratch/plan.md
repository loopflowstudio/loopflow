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

History: `scratch/plan-ownership-history.md`. Concept review (2026-11-18):
`scratch/concept-review.html`.

## The design in one paragraph

The hub holds the plan in the store's own words. A Machine whose plan is on a
hub saves locally as before and queues each changed value, a `WorkEdit`
(target + `WorkValue`), in `hub_sends`; an exchange sends a repository's open
values in `plan.apply` and the hub writes them with the same typed writer.
Inbound, `plan.changes` returns the `WorkEdit`s of owners changed since a
watermark, and the Machine writes them as an import. No translation, no
`decide`, no tracker identity binding: both ends share every id and field. On
the hub Machine the store *is* the plan and nothing is queued. The Git
planning ref, the peer journal, field causality, destinations and the
per-Session exchange go in slice 2. `lf repo tracker` reads
`Plan: hub mini. 3 open.`

## Concept review changes (2026-11-18, made for review)

The kickoff plan made the hub "a third tracker": `Tracker::Hub` beside Linear
and Asana, local saves queued as tracker actions, inbound through
`planning::rules::decide`. The review replaced that:

1. **The hub is not a tracker; it is the plan's home elsewhere.** A tracker is
   a foreign system reached through a lens: `ActionKind::of` keeps only the
   values Linear/Asana can hold, `enqueue_local` queues only `tasks`,
   `projects` and `task_comments`, and `Tracker::changes` answers in
   `TrackerItem` words. Through that lens the hub would never learn any Wave
   field, `project_wave`, ranks, `task_shipped_at`, `task_due_date`,
   `task_branch`, `task_url` or `task_identifier`, all of which the Git
   journal carries today (`planning_write::capture` records every
   `WorkValue`). So the hub queues every `WorkEdit` the writer saves, and is
   not a `Tracker` variant: its `look`, `changes`, `fetch_issue`, `removed`
   and `issue_ownership` would all be unreachable.
2. **One rule, the one people already have: the change sent last wins.**
   The kickoff plan added a per-field sequence check that marks an offline
   edit `lost` when the hub moved since its base. `decide` never does that
   for an unsent value ("Unsent values go out even over a provider edit: the
   change sent last wins"), and the Git journal's latest clock gives the same
   answer in the acceptance sequence. The hub applies in arrival order;
   setting a value twice is the same value, so every hub send is safe to
   repeat after a lost reply, and the hub never produces `conflicted`.
   Keep / Use mine stays Linear/Asana's. Passed over: a hub-side conflict
   check (it would be a second conflict rule with its own table, for a
   behaviour nobody asked for). Counterexample kept: two people editing one
   title while one is offline lose the earlier edit silently, as with Linear
   today.
3. **No create/state/order/delete/link kinds for the hub.** A new Task is its
   first `WorkEdit`s (the writer's `ensure_record` creates the row, as Git
   imports do now); deletion is `task_deleted_at`; order is `task_rank` /
   `project_rank`; a comment is `comment_task` + `comment_content`. So the
   "every kind" slice disappears and slice 1 carries every field at once.
4. **No binding.** `external_issue_id` stays a Linear/Asana column; hub rows
   share ids by construction.
5. **First contact pulls before it pushes.** Pushing a Machine's whole plan
   first would overwrite mini's newer values with stale ones under "sent last
   wins". So: `plan.changes(None)` first, imported; then queue edits only for
   owners the hub did not return.

## Findings that shaped it

- **The carrier exists.** #1956 (LOO-926) sends typed requests to another
  Machine's server with `ssh <machine> lf server connect`, protocol revision 8,
  version-skew errors naming the fix. The hub client uses that carrier now;
  LOO-861 swaps it for the hub's persistent outbound connection later without
  changing the operations. **So LOO-938 does not wait for LOO-861**, which is
  still unstarted.
- **The writer already speaks both directions.** `planning_write::write(conn,
  edits, origin)` takes `WorkEdit`s; `WriteOrigin::Import` writes without
  queueing or echo (today's Git import uses it); `ensure_record` creates
  missing Waves, Projects, Tasks and comments from their edits.
- **"Git-planned" is the default, not a choice.** `TrackerConfig::Git` is what
  a repository with no `tracker:` key gets (`config.rs:315`, `GitPlan::Shared`
  default). etude has no `tracker:` key. So the new rule needs no repository
  key: no Linear/Asana → the plan is on this Machine's hub, or local when it
  has none (LOO-406's store-is-home, unchanged for a single Machine).
- **etude is the only live user.** `lf repo tracker` in `~/src/etude`:
  `shared/main` 397 records, publication confirmed; `users/9a0c1265…` 22
  records, "local changes pending" and "Git acquisition failed". Git Tasks
  carry no identifiers (`identifier: null`) and are addressed by UUID.
- **mini was unreachable during kickoff** (`ssh mini`: hostname does not
  resolve; `100.96.227.95:22` timed out). Whether mini has an etude checkout
  and a current `lf` is unverified; slice 2's cutover checks it.

## Shape

### Which Machine is the hub

A Machine stores one hub: `lf machine hub mini` names a saved Machine
(`lf machine add` already saves them); `lf machine hub --clear` removes it;
`lf machine list` marks it. LOO-861 later owns this same setting as its "one
hub address". A Machine whose own `machine_id` equals its hub's id, or with no
hub, is authoritative for its own plan. *Made for review:* a Machine-level
setting rather than a repository `tracker: hub` key, because who the hub is
depends on the person's Machines, not the repository.

### What exists (slice 1, 2026-11-18)

- `lf machine hub <machine>` / `--clear` / bare writes the `hub:` key in
  `~/.lf/config.yaml`; `lf machine list` marks it. `ops/plan_hub.rs::hub`
  resolves it (`None` when unset or when the hub is this Machine).
- Migration `plan_hub.sql`: `plan_changes` (one row per owner, `seq` only
  grows, seeded with every owner so any Machine can become a hub),
  `hub_plans` (per repository: hub id, watermark `seq`, NULL = first contact,
  last error), `hub_sends` (newest unsent value per (target, field)).
  `sqlite/plan_hub.rs` owns them.
- `planning_write::write_record` calls `plan_hub::saved` for every owner it
  changes: bumps `plan_changes`, and when a `hub_plans` row exists, queues the
  changed values (a created record whole).
- Server protocol revision 9 (`PLAN_REVISION`; Swift `ServerProtocol.revision`
  9): `plan.changes { repository, since }` → `{ edits, seq, more }`, paged
  under half a control message; `plan.apply { repository, edits }` → `{}`,
  written in one transaction, safe to repeat.
- `plan_hub::exchange`: read every page first, import (skipping fields with
  an open send), then send grouped by owner. Called from
  `planning_sync::exchange_repository`, i.e. at every Git exchange boundary;
  a publish-only boundary with nothing open asks the hub nothing.
- `lf repo tracker` prints `Plan: hub mini[, unreachable since …]. N open.`
  with each open send; a Wave rename syncs after its save.

## User-visible outcome

On etude, from the laptop: `lf task create 'x'` returns at once; `lf repo
tracker` shows `hub mini · 1 open` then `0 open` after the drain; on mini,
`lf task list` shows the Task. A Wave renamed on the laptop is renamed on
mini. Laptop offline: create and edit work, show open, and arrive on
reconnect; the value sent last is the one both Machines show. Desktop loses
the Git destinations list; its tracker line reads like a Linear
repository's.

## Acceptance check at gate

Headless, two homes on one Mac with #1956's stand-in `ssh`
(`.lf/qa/machine-moves.md`), home B's hub = home A:

```bash
B: lf task create 'hub smoke' --json          # B: lf repo tracker → "1 open"
B: lf task sync <id>                           # → "0 open"
A: lf task show <id> --json                    # title "hub smoke"
A: lf task edit <id> --title one ; stop A's server
B: lf task edit <id> --title two               # "hub … unreachable · 1 open"
start A's server; B: lf task sync <id>         # → "0 open"
A: lf task show <id> --json                    # title "two"
B: lf task show <id> --json                    # title "two"
A: rename a Wave; B: lf task sync; B shows the new name
```

Plus a Rust integration test of the same sequence with the carrier mocked at
the `lf server connect` pipe, including a lost `plan.apply` reply that is
sent again and applied once.

## Affected surfaces

- `plan_hub.sql` gains the `-- leaving:` marks (slice 2).
- Server protocol + `tests/fixtures/dto/server_schema.json`, Swift
  `ServerProtocol`, Pi extension, `server_connection.js` (revision bump only).
- `lf repo tracker` / `TrackerActionStatus`: hub reachability line replaces
  destination receipts; `lf repo tracker journal` goes.
- `lf machine hub`, `lf machine list`.
- `.lf/config.yaml`: `tracker: { git: … }` is a shared file the released `lf`
  reads; the branch parses it and treats it as "no tracker", marked with a
  removal Task filed in this change (AGENTS.md shared-file rule).
- Docs: `docs/dev/planning.md` (Git sections), `docs/dev/machines.md`,
  `docs/glossary.md` ("destination"; "tracker" now means Linear or Asana,
  and the hub is where the plan lives), `AGENTS.md` work model line
  ("Linear, Asana or a Git ref is the repository's tracker"),
  `rust/loopflow/src/server/README.md`, `rust/loopflow-storage/SCHEMA.md`.

## What goes

- `planning_git.rs` (437 lines); Git half of `ops/planning_sync.rs`
  (`fetch_repository`, `publish_repository*`, `exchange_destination`,
  `fetch_destination`, `publish_destination`, `wait_for_publication`,
  `git_destinations`, `git_journal`, `adopt_personal_key`,
  `configured_destination`, `defer_exchange`, `PlanningSync` and its three
  starters: `harness/native_session.rs:526`, `agent/mod.rs:1019`,
  `transport/hosted.rs:101,904`).
- `loopflow-storage`: `git_tracker.rs`, `sqlite/git_tracker.rs` (1116),
  `planning_exchange.rs` (`PlanningMutation`, causal parents),
  `planning_destination.rs`, journal parts of `planning_sync.rs` /
  `sqlite/planning_sync.rs`, `planning_write::capture` / `retain` /
  `settle` (journal recording), `execution/store/git_tracker.rs`,
  `WriteOrigin::Import`'s "Git peer" meaning (it becomes "the hub's value").
- `TrackerConfig::Git`, `GitPlan` (`Shared`/`Personal`) after the marked
  fallback.
- Tests: `tests/planning_git_tests.rs`, `planning_reconnect_tests.rs`,
  `tests/e2e/planning_reconnect.py`, Git parts of `planning_setup_tests.rs`,
  `planning_foreground_tests.rs`, `store/planning_sync_tests.rs`,
  `planning_exchange_lifecycle_tests.rs`.
- Swift: `GitDestination.swift`, `GitDestinationsView.swift`,
  `PeerPlanning.swift` and their DTO fixture `git_destinations.json`.
- PR #1971 and LOO-899 / LOO-868 patches go with the code they patched.

Survives: every Wave, Project, Task and comment id and value, ownership
entries; the Linear/Asana `tracker_actions` queue; Machine-local Sessions, Processes,
checkouts, reservations (Infrastructure Wave memory "Plan ownership").

## Cutover for etude

Not code (no cross-repo migrations). Before installing: with the released
`lf`, `lf task sync` on every Machine until `lf repo tracker` shows both refs
confirmed, then on mini. Install. `lf machine hub mini` on the laptop; first
contact imports `plan.changes(None)`, then queues edits for owners the hub
lacked (the 22 personal records if mini never fetched them). The ref stays on
origin, inert; deleting it is one hand-run
`git push origin :refs/loopflow/planning/…`.

## Exclusions

- Linear/Asana drains stay per Machine (LOO-925). Moving them to the hub is a
  later question (`scratch/questions.md`).
- The persistent hub connection, presence, phone pairing: LOO-861.
- Task identifiers for hub repositories (Tasks stay UUID-addressed).
- Several people on one hub: arrival order, no admission beyond SSH-as-owner,
  as #1956.

## What slice 1 taught

- **`hub_sends` is its own table, not `tracker_actions` rows.**
  `tracker_actions.tracker` is `CHECK(tracker IN ('linear','asana'))`: a
  `hub` row needs a table rebuild (with its canonical-identity triggers), and
  a released lf's `TrackerKind::parse` fails on it. Concept review point 1
  says the same. A confirmed send deletes its rows.
- **The watermark row is the switch.** A `hub_plans` row makes the writer
  queue; it is made at the first exchange, so saves before it reach the hub
  through first contact.
- **A clone on the hub mints its own repository identity.** The hub needs
  `lf config repo --bind <id>` in its clone; the error names it.
- **Sends group by owner,** so a record the hub lacks arrives with every
  value `ensure_record` needs.
- The open-row skip held: whole-owner imports did not fight local edits in
  the two-home run, so the inbound shape stands.
- Two-home run (stand-in `ssh`, `/tmp` homes): a Task made before the hub was
  set reached mini by first contact; `lf task create` and `lf task edit`
  sent at once; with mini down, `lf repo tracker` read `Plan: hub mini,
  unreachable since … 1 open.` with the ssh reason, and the edit sent last
  (`two`) won on both after mini came back; a Wave made and renamed on mini
  appeared on the laptop after `lf task sync`; a Wave renamed on the laptop
  reached mini at once. A swapped hub binary answered "Loopflow server uses
  another executable".

## What remains: slice 2, delete Git

1. **Give the hub exchange its own call sites.** Today it rides
   `planning_sync::exchange_repository`, which goes with Git. Reads come only
   at the Git boundaries (`lf task sync`, a cold Task lookup, a foreground
   provider's loop, `lf wave status --sync`); `docs/work.md` says "Each `lf`
   command sends what is open and reads what changed on the hub". Sends hold
   that; reads must too.
2. **Delete everything in "What goes"**, with `-- leaving:` marks in
   `plan_hub.sql` for every `git_tracker_*` table and the `planning_peer_*` /
   `planning_destinations` copies (and `planning_user` if nothing else reads
   it). `WriteOrigin::Import` then means only "the hub's value".
3. **`lf repo tracker --json` / `TrackerStatus` and Desktop's tracker line**
   report the hub (they still report Git destinations with a hub set);
   `Plan: this Machine (hub)` on the hub, which today prints the Git status.
4. **`lf task sync` output**: prints `<id>: synchronized`
   (`ops/task.rs:2640`); `docs/work.md:245` shows `nothing open`.
5. **`.lf/config.yaml` `tracker: { git: … }`**: parsed as "no tracker",
   marked with a removal Task filed in this change.
6. **Docs** in "Affected surfaces" that still describe Git.
7. **Rust integration test** of the acceptance sequence with the carrier
   mocked at the `lf server connect` pipe, including a lost `plan.apply`
   reply sent again (store tests cover the sequence; the two-home run covered
   the carrier).
8. **etude cutover against mini** (below), by hand.

Known and left: a foreground provider's loop opens `ssh … lf server connect`
every 5 s to read; LOO-861's persistent connection replaces it.

Checks: focused Rust tests per slice; `lf repo tracker` on etude before and
after cutover (gate, by hand); Desktop view tests for the tracker line (CI).

## User docs (written first, 2026-11-18)

`docs/work.md` ("Share the plan", Sync reference, `--subscribe` line),
`docs/ops.md` (Other machines), `docs/config.md` (Tracker row),
`docs/glossary.md` (Tracker, Hub, Linear). Choices made while writing:
`lf repo tracker` heads with `Plan: hub mini. N open.` /
`Plan: hub mini, unreachable since 14:02. N open.` / `Plan: this Machine (hub)`;
hub lines are `<task-id>  <field> · queued · <send error>` and
`wave <name>  <field> · …`; a hub without the repository's checkout keeps sends
open with "the hub has no checkout". `documented_commands` will flag
`lf machine hub` until slice 1 adds the command.

Open question: today a coworker who clones a repository shares its Git plan
with no account. After slice 2, a repository without Linear or Asana shares
its plan only among one person's Machines (the hub is reached over SSH as
you). Is losing zero-setup coworker sharing intended, or should a hub admit
other people?
