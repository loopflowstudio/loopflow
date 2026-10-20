# LOO-523: Own Tasks as (person, Machine) with an append-only ownership log

Status: ownership log and personal Wave Machine implemented (2026-10-19/20);
Jack Heart's 2026-10-20 direction (repo Machine, personal Machine registry) is
remaining work. Jack's brief (Task description) holds the accepted decisions. This plan holds the mechanisms;
choices that need Jack's judgment are in `scratch/questions.md` (items 7-12 record
where implementation departed from this plan).

## Problem

Three half-overlapping facts answer "who is doing this, where?" and none of them
travels:

- `tasks.started_at` is a write-once column that SQL triggers enforce, and it
  never leaves the Machine.
- Placement (`work_placements`, plus `Wave/Project/TaskDelegation` WorkValues)
  inherits a Machine through Task → Project → Wave. It travels only in Git mode,
  and with no assignment every Machine resolves `local_default` to itself.
- First-start admission (`tasks.admission_authority`, `task_admissions`) makes
  the *creating* Machine a permanent coordinator. Starting a laptop-created Task
  on the mini therefore needs SSH up to the laptop, and an admission can never
  be handed off or unclaimed.

Linear never learns who took the Task. A reopened PR silently moves a Done Task
back to Linear `unstarted`. Jack Heart: first-start authority "doesn't sound like
a real thing that should exist."

## Demo

The demo runs on the laptop, in this Linear-mode repository, on a throwaway Task:

```
lf task create "ownership demo"        # status: Unowned
lf task run <id>                       # claims, then runs: "Jack Heart on <laptop>"
lf task status <id>                    # ownership.owner + log[claim]; Linear assignee = Jack
lf task handoff <id> mini              # log[claim, handoff → mini]; owner "Jack Heart on mini"
lf task unclaim <id>                   # routes over `lf --machine mini`; writes unclaim there
lf task status <id>                    # Unowned; log keeps claim, handoff, unclaim; Linear unassigned
```

Desktop shows the same owner line in the Task row ("Jack Heart on mini" or
"Unowned"), and the log in Task detail. If mini is unreachable, the fallback
reverses the last two steps: unclaim before handoff, then hand off to a
registered fixture Machine id. A handoff needs only the target's identity, not
a connection to it.

## Principle (goes to the top of docs/architecture-reference.md)

Intent flows down and execution facts flow up. Intent means Tasks, Waves,
Projects, Workflow moves, ownership, pools and budgets. It has one owner of
record: the planning provider, or Loopflow's journal when the plan is local.
Execution facts are Processes, Sessions, FlowProcess rows and checkouts. The
Machine that ran them owns them. Upward copies (worker → controller → laptop)
are read-only and keep their origin; they become the final record only when
the origin is gone. The controller aggregates execution and is not the root.
There are no daemons or listeners; `lf --machine` over SSH is the only
transport between Machines.

## Chosen approach

### Model

`TaskOwnershipEntry { id: OwnershipEntryId ("own_…"), task_id, kind, person:
Option<String>, machine_id: Option<MachineId>, at }`

- The kinds are `claim`, `handoff` and `unclaim`. A fourth kind, `recorded`,
  exists only for migration: it holds what was known before this change.
- **The later entry wins** (Jack Heart, 2026-10-19). The log follows the
  provider's order. In Linear, that is the comment `createdAt`. In Git, it is
  the journal's existing causal order, with clock and mutation-ID ties. Entries
  not yet delivered sort after every delivered one, by local `at`.
- The owner is the last entry, unless that entry is an `unclaim`.
  `TaskOwner { person, machine: MachineRef { id, label } }`, where a missing
  `machine_id` displays as "Machine unknown".
- There is no contested state and no `supersedes` link. If two workers claim,
  the second claim wins.
- Losing ownership cancels nothing, and on its own Machine the earlier
  claimant remains the owner (Jack Heart, 2026-10-19). An inbound entry never
  displaces the local Machine's own open claim. Locally, `owner` stays "you on
  this Machine", and runs and routing continue there. Every other Machine
  follows provider order and shows the later claim.
- The earlier claimant finds out on its next ownership refresh. It records the
  later claim as `superseded_by: Option<TaskOwner>` beside `owner`:
  - `lf task status` reports "also claimed later by Kim on mini".
  - The Session context block carries the same line while this Machine has live
    work.

  It then decides: keep going, or `lf task unclaim` or `lf task handoff`, which
  ends its local claim. Re-claiming makes its claim the later one everywhere.
- The table is `task_ownership`, append-only. Triggers abort UPDATE and DELETE,
  following the `task_workflow_moves` precedent. Rows outlive Task deletion,
  and a re-import cannot rewrite them.
- `person` is `config::participant_name()` at write time. `LF_USER_NAME`
  already crosses `lf --machine`, so a claim the laptop routes to mini records
  Jack.

### Wave Machine (replaces Placement; personal, never repository content)

Jack Heart (2026-10-20): everyone in a repository has a repo Session, and everyone
in a Wave has a Wave Session for it. Which Machine runs them is that person's
choice. Nothing about Machines is committed: delete the `.lf/config.yaml`
`machines:` block, `MachinesConfig` and its resolver.

```
lf wave machine infrastructure mini   # your default for this Wave's Tasks
```

- Stored in this Machine's store as the person's Wave → Machine default, and
  copied to the person's other registered Machines over `lf --machine`, so the
  laptop knows to launch there and mini knows to receive the runs.
- Setting it moves the person's Wave Session to that Machine, restarting it
  there if it does not already live there.
- Resolution for a new Task: its Wave's default → parent Waves → the person's
  repo Machine → this Machine (Jack Heart, 2026-10-20). No setting means this
  Machine. (Built: Wave → parent Waves → this Machine; the repo step is remaining.)
- Captured, not inherited: a Task's Machine is the one in its claim. Changing
  the Wave default later moves no existing Task; handoff is how a Task moves.

### Repo Machine (Jack Heart, 2026-10-20; remaining)

```
lf repo machine mini     # your default for the repo Session and Tasks with no Wave
```

- Mirrors `lf wave machine`: keyed by repository, stored in this Machine's
  personal store, copied to the person's other registered Machines over
  `lf --machine`.
- Setting it moves the person's repo Session there: complete the old primary,
  admit on the target.
- `lf connect --repo` and repo `--fresh`/Desktop `resetSession` route to it.

### Machine registry is personal (Jack Heart, 2026-10-20; remaining)

The set of available Machines (`lf machine add/list/remove`, labels, routes)
belongs to the person across all their repositories: no repository column or
repo-keyed lookup on `machines`, and no repository content. Wave and repo
Machine defaults stay keyed by Wave or repository in the personal store.

- Today `machines.repo` is the remote checkout path: `lf machine add` stores it,
  `remove_machine` clears it with the label (`store/sqlite/durable.rs:131`), and
  `ssh.rs:184-190` uses it as the remote repo when no repository is named.
- Sketch: drop the column in the draft; `lf --machine` names the repository by
  identity (`--repository`, as Wave Machine copies already do) and the remote
  resolves its own checkout.

### Writer rule (forwarding stays within one person's Machines)

- Owned by you and its Machine is another of your Machines: the command reruns
  there through `work_route` (`lf --machine <machine>`).
- Unowned: your claim is written on your Wave Machine for that Task (forwarded
  there when it is not this Machine).
- Owned by someone else: never route to their Machine. A claim is written on
  your own Machine; the later claim wins, as decided.

The Machine in the Linear claim comment is informational: public by default,
shown after the person; nothing another person does depends on it.
The local append runs in one IMMEDIATE transaction.

Writers:

- `place_task`, the checkout allocation used by `lf task run` and
  `lf task checkout`, claims an unowned Task in the same transaction. This is
  the "actual work reservation" that used to set Started.
- `lf task claim`, `lf task handoff <task> <machine>` and `lf task unclaim`
  are explicit. A handoff target may be any well-formed MachineId or a
  registered label. A handoff keeps the person.
- Completion, abandonment and reopening write no entries. A Done Task shows
  its last owner as history.

### Primary Sessions follow the Machine (Jack Heart asked, 2026-10-20)

Today only `--task` connects route (global `--task` → `work_route::dispatch` →
owner). Primary pointers (`agent_sessions.primary_scope`, `tasks.primary_session_id`)
are per-Machine, so each Machine can hold its own "one" primary. Decisions:

- **Wave Sessions route to your Wave Machine.** `lf connect --wave`, context
  connects resolving to a Wave, and `--fresh`/Desktop `resetSession --wave` route
  through `lf --machine` to the personal Wave Machine (this Machine when unset).
  `--repo` routes to the personal repo Machine once it exists (remaining).
- **Moving a Wave Session.** `lf wave machine <wave> <m>` completes the source
  primary on the old Machine and admits a successor on `<m>` through the existing
  `ensure_primary_session_in` replace path. Native provider history stays on the
  old Machine (it lives in that Machine's files); the successor gets fresh Wave
  context, like `--fresh`.
- **Task primaries follow the claim.** A handoff completes the Task's primary on
  the source Machine; the target admits a fresh one on first connect. One primary
  per scope across a person's Machines.
- **One location source.** Routing and location reads both use the ownership
  Machine: `observe_task_location` stops requiring `checkout_machine_id == local`
  on the owner; opening a claimed Task with no checkout on its Machine creates it
  there (`ensure_task` already runs on the routed owner).
- **No unclaimed local checkouts.** `ensure_task`/`admit_workspace` reached from a
  context connect route to the owner like `--task`; on an unowned Task they claim
  (actual work reservation), so a checkout never appears on a non-owner Machine.
- **`lf open` reads remote primaries.** `primary` and `elsewhere` liveness for a
  remote Task come from the owner Machine, not the local store.
- **Docs:** creation no longer admits a primary (`docs/architecture-reference.md:333`,
  `README.md:303`); say primaries are admitted lazily on the owner Machine.

### Transport (ownership must travel in both modes)

**Git planning (as built):** the same marked comment as Linear. The comment is
already a journal record; import parses the marker into `task_ownership` with
INSERT OR IGNORE, and the comment time is the order. Each local append sorts after
every entry its Machine has seen. (Planned: a dedicated WorkValue; see
`scratch/questions.md` item 7.)

**Linear:** delivering an entry makes three foreground effects, each with the
existing receipts:

1. A Task comment, "Jack Heart claimed this on mini" (or "handed off to …" /
   "unclaimed"), followed by `<!-- loopflow-ownership:{entry json} -->`. It uses
   the existing comment delivery. Inbound comment refresh parses the marker into
   `task_ownership` (INSERT OR IGNORE), the same way steers already ride on
   marked comments. `is_direction_comment` excludes the marker.
2. `TaskAssignee`: set to the Linear viewer id on claim, left unchanged on
   handoff, cleared on unclaim. The viewer id is cached per Machine, and the
   query already exists (`linear/mod.rs:590`). If it can't be resolved, the
   assignee is left alone with a warning; the entry still saves.
3. On a claim, if the planning state is `unstarted` or backlog, the existing
   state delivery queues `started`. This resolves finding 3.

Linear can never push an unassignment to a Machine that thinks it owns the Task
(Jack Heart, 2026-10-19). Inbound observations never end a local open claim.
This covers assignee clears and changes, state changes, and ownership comments
written elsewhere. Only that Machine's own `unclaim` or `handoff` ends the
claim. An inbound assignee change writes no ownership entry.

The owner re-asserts itself (Jack Heart, 2026-10-19). When a Machine holds an
open claim and observes that Linear's assignee no longer matches it, the
Machine tells Linear again: it queues `TaskAssignee` for its cached viewer id
through the same field delivery. This applies whether the assignee was cleared
or changed. It re-asserts only while it has seen no later claim (no
`superseded_by`). An earlier claimant keeps its local ownership but stops
contesting Linear. It writes no new ownership entry. It runs on the observation that
detects the mismatch (comment or issue refresh), with no daemon. The existing
receipts already deduplicate a pending delivery. Together with the
`superseded_by` condition, this means two claimants never alternate the
assignee.

Git works the same way. An owner re-asserts whenever the provider's ordered log
stops showing it as owner for any reason other than a later claim. The usual
case is a stale Machine writing a `unclaim` or `handoff` of a claim it did not
own. In Git, re-asserting appends a fresh `claim` entry, which is then the later
one. In Linear, it re-queues the assignee and, if the last ownership comment is
someone else's, appends a claim comment. Either way it is one rule in one
place: the ownership refresh that sets `superseded_by`. A person who wants the Task off that Machine unclaims it
there.

### Consumers cut over

- **Routing:** `task_execution_route` returns `Owner(machine)` or
  `OwningMachine(machine, source)`. `RecordedCheckout`, `Admission`,
  `EffectiveDelegation` and the shared-Task peer scan in `task_location::resolve`
  are deleted. `lf task location --peers` stays as an observation only.
- **Started users:**
  - Chapter rotation carries owned Tasks (`chapters.rs task_started` /
    `chapter_task_evidence`).
  - Work selection (`work/selection.rs:59`) uses owned.
  - `require_current_task_project` exempts owned Tasks.
  - CI cycle time starts at the first claim's `at`.
- **DTOs:**
  - `TaskStatus` gains `ownership: TaskOwnership { owner: Option<TaskOwner>,
    superseded_by: Option<TaskOwner>, log: Vec<TaskOwnershipEntry> }`.
  - In `lf wave show`, `TaskRuntimeSnapshot.started: bool` is replaced by
    `owner: Option<TaskOwner>`.
  - The Wave snapshot's `machine` comes from the resolver.
  - Fixtures change: `task_status.json`, `task_delivery_rows.json`,
    `wave_detail.json`, `roadmap_snapshot.json` and `unplaced_wave.json`.
- **Swift:**
  - `TaskRuntime.started` becomes `owner`.
  - `TaskRowMetadata` shows "person on machine" or "Unowned".
  - `WorkSurfaceView` groups by Owned / Unowned.
  - Task detail lists the log.
  - The local Machine's label is the short hostname, read at display time. The
    local row has no name today.
- **Finding 4:** delete the `queue_in(…, "unstarted")` in `insert_task_pr`
  (`children.rs:617-619`). A new PR on a Done Task leaves it Done until someone
  runs `lf task reopen`.

### Migration (single draft `task_ownership`)

- Create `task_ownership` and its triggers.
- Write one `recorded` entry for each Task that has `started_at`, a worktree or
  a `task_admissions` row:
  - `machine_id = coalesce(task_admissions.machine_id, checkout_machine_id)`.
    Otherwise NULL, meaning "Machine unknown".
  - `person` is NULL.
  - `at = coalesce(started_at, created_at)`.
- `recorded` entries are never exported or delivered. Each Machine's local
  evidence stays local, and later entries supersede it.
- Drop the started triggers (`validate_task_started_*`,
  `task_first_conversation_*`, `task_first_agent_turn`), the admission and
  placement canonical/revision/immutability triggers, `task_admissions`,
  `work_placements`, `tasks.admission_authority` and `tasks.started_at`.
- Delete local journal mutations whose field is `task_admission_authority` or
  `*_delegation`. Jack's store holds 1,466 of the 17,710 in
  `planning_peer_changes`. Retired values have no decoder.
- Released SQL and the `canonical_ids_work.sql` fixture stay untouched. The
  upgrade test seeds that released frontier and asserts the entries.

Jack's laptop store holds 533 placements (all `legacy`, all on this Machine),
58 admissions and 294 started Tasks. No explicit placement is lost.

## Risk findings

- **Retired WorkValue decoding:** Git documents from an un-upgraded peer that
  still carry `*_delegation`/admission values will fail import (whole-document
  rollback), consistent with Jack's no-old-peer rule. Implement must confirm
  that deleting local journal rows leaves no dangling causal parent that blocks
  export. Probe: run the draft against a copy of `~/.lf/loopflow.db`, then run
  the planning export/import tests.
- **`started_at` DROP COLUMN:** SQLite refuses while any trigger, index or view
  names the column. Every dependent trigger in 0.14.6/0.14.8 must drop first.
  `migrations.rs` asserts on started at 2270, 3275 and 6383; those are rewritten
  to the released frontier.
- **The brief's line numbers:** `children.rs:343/398/618` and
  `durable.rs:219/252` are Linear state-delivery calls, not Started. Finding 3 is
  resolved by claim delivery (assignee + `started` state), not by editing them.
- **"`lf machine remove` loses the only copy":** removal only clears label and
  repo (`store/sqlite/durable.rs:129-139`). The real loss is a destroyed worker,
  and that is the upward-copy follow-up.
- **No local Machine name:** the hostname is used at read time. No column is
  added.
- **Copy freshness:** a Wave/repo Machine copy that fails to reach an
  unreachable Machine leaves it resolving its own default until set again there.

## Alternatives rejected

- **Keep Placement in the journal and add a Linear transport for it.** This
  needs a second, Loopflow-only channel into Linear for every Wave field.
  Config already travels in both modes.
- **Ownership only as marked comments, with no table.** The log would then be
  a parse of mutable comment bodies. Migration entries would be delivered from
  every Machine and fork.
- **Run the Git journal in Linear mode for Loopflow-only fields.** That makes a
  second owner of record for intent, and LOO-412 rejected a parallel transport.
- **Grant records or leases.** These are the admission model under another
  name. Single-writer routing makes them unnecessary.

## Exclusions → follow-up Tasks (file during gate)

- **Upward copies:** origin Machine and observed-at on Process, Session,
  FlowProcess and session-event rows, plus a controller pull path. Stamping
  without a pull path would be dead data. The ownership demo needs neither.
- **Cross-person handoff:** this needs a Machine → person registry and
  resolving the person to a Linear user. Today a person unclaims and the other
  person claims.

## Delete — do not maintain

- **Admission:**
  - `ops/task_admission.rs`, `store/sqlite/task_admission.rs`
  - `TaskAdmission`, `TaskAdmissionAuthority`, `TaskExecutionSource::{Admission,
    RecordedCheckout, EffectiveDelegation}`
  - `WorkValue::TaskAdmissionAuthority`
  - the `children.rs:122-131` stamp, `planning_write.rs:276-285`, and
    `require_task_planning`'s shared refusal (`children.rs:1137-1149`)
  - `lf task admission` (`lf/mod.rs:1276`, `bin/lf.rs:913,1626`), and the
    Admission special cases in `ssh.rs:28,41` and `work_route.rs:55,113`
  - the admission notes in `task_location.rs:132-187` and `explain.rs:103`
  - the tests at `task_admission.rs:202-411` and `planning_write.rs:659-710`
  - the admission parts of `machine_commands.rs:1662` and
    `global_commands.rs:3329`
  - the admission string in `task_checkout_explanations.json` and the Swift
    test `taskCheckoutExplanationsKeepProposalsSeparateFromAdmission`
- **Placement:**
  - `work_placements`, the `Delegation` struct and `Wave/Project/TaskDelegation`
    WorkValues
  - `Placement`, `PlacementProvenance`, `placement_in`, `find_placement`,
    `place_work`
  - delegation routing in `planning_write.rs`
  - `lf wave place` and `lf/commands/placement.rs`; replaced by `lf wave machine`
  - `tests/fixtures/dto/placements.json` (unused)
  - the placement tests in `durable.rs:920-1190`, `store/durable.rs:198-238` and
    `ops/task.rs:5188-5392`
  - `repo_operate.md:287-295`
- **Started:**
  - `tasks.started_at` and its triggers
  - `flow_inventory.rs:211-214`
  - `TaskRuntimeSnapshot.started`, `TaskStartEvidence`'s started_at arm
  - Swift `TaskRuntime.started`
- **Requeue:** `insert_task_pr`'s Done → `unstarted` requeue.
- **Docs:**
  - `machines.md:69-73,79-96,110-114,134-160`
  - `architecture-reference.md:615,622,633,1203`
  - `data.md:109-132`, `planning.md:163-166,185-190`
  - `lf.md:133-146,762,774`, `lf-reference.md:1545,1754-1773`
  - `AGENTS.md`'s Started sentence ("Actual work reservation sets Started
    once")
  - `docs/data-clones.md:26`
- **Must survive:** `checkout_machine_id` and the worktree columns (local
  execution facts), Session bind, Workflow moves, `lf task location` as an
  observation, `lf machine` and `--machine`, and Linear assignee edits through
  `lf task edit`.

## Forbidden outcomes

- Placement or `started_at` kept beside ownership.
- Any "creating Machine" authority.
- A recorded entry that invents a person or Machine.
- Ownership entries that can be updated or deleted.
- A decoder for retired WorkValues.
- Peer-scan routing kept as a fallback.
- Linear assignee as a second ownership store.
- Any inbound observation (Linear or Git) ending or transferring this Machine's
  own open claim.

## Internal slices (one PR)

1. **This slice:** the draft migration plus the store layer, which deletes
   admission, placement and `started_at`. It adds `task_ownership` with
   append/read/ordered-owner logic, the writer rule, the Wave Machine resolver, and
   `place_task` claiming. Focused test:
   `cargo test -p loopflow ownership` (store unit tests for claim → handoff →
   unclaim, wrong-writer refusal, a later delivered claim winning, and
   migration from the released frontier).
2. Routing and CLI: `task_execution_route`, `claim`/`handoff`/`unclaim` with
   `work_route` forwarding, and deletion of the peer-scan and admission CLI.
3. Transport: the Git WorkValue, Linear comment + assignee + state delivery,
   and inbound parse.
4. Consumers: status, wave show and the DTO fixtures, Swift, chapters,
   selection, CI, and the `insert_task_pr` fix.
5. Docs: the principle at the top of `architecture-reference.md` and the
   rewrites listed above.

## Done when

- `lf task status <id> --json` on an unowned Task shows `ownership.owner: null`
  with an empty log.
- After `lf task run`, it shows a `claim` with Jack and this Machine, and Linear
  shows Jack as assignee and the state started.
- `handoff` appends an entry superseding the claim. A run on the old Machine
  routes to the new owner instead of allocating a checkout.
- `unclaim`, initiated from the laptop, executes on the owner, clears the Linear
  assignee, and leaves a three-entry log.
- While a Task is claimed, clearing its assignee in Linear leaves the claim
  open, and the next refresh on the owner restores the assignee. In Git, a
  stale Machine's unclaim of that claim is followed, after exchange, by the
  owner's fresh claim.
- Two of your Machines that share Git planning and the same Wave Machine
  cannot both claim: the non-owning one forwards to the owner. With no
  Wave Machine set, both claims are delivered and, after exchange, each
  other Machine shows the second claim as owner. The first claimant still shows
  itself as owner, with `superseded_by` naming the second. Its running Flow is
  untouched, and its context block reports the later claim. The integration test covers this
  with two `LF_HOME`s and a stub SSH.
- An upgraded store from the released frontier holds one `recorded` entry for
  each started Task, with "Machine unknown" where the Machine was unknown, and
  no admission, placement or `started_at` schema.
- Desktop row and detail show the owner, or "Unowned".

## Acceptance at gate (headless)

```
cargo test -p loopflow --test integration task_ownership   # two-Home claim/forward/handoff/unclaim, Git exchange
cargo test -p loopflow ownership                           # store + migration
cargo test -p loopflow --test dto_fixtures                 # status / wave show shapes
uv run python scripts/check_migrations.py
xcodebuild test … -only-testing:LoopflowTests/DTOFixtureTests  # owner decoding (CI)
```

The live Linear demo above is a demo/review step, not gate.

## Remaining work

- `lf repo machine` and the repo step in Wave resolution (above).
- Personal Machine registry: drop `machines.repo` and repo-keyed routing (above).
- Context-resolved connects, owner-Machine `observe_task_location`, and remote
  primaries in `lf open` (`scratch/questions.md` item 18).
- Desktop Task detail lists the log (needs the log in a Desktop-read DTO).
- Live Linear demo on a throwaway Task (demo/review step).
- File the follow-up Tasks named under Exclusions during gate.

Realign (2026-10-20): `observe_task_location` read the dropped `tasks.started_at`
(every reading fell to Unavailable); it now reads ownership. The main merge left
`world/mod.rs` calling the old three-argument `record_flow_process`; fixed.
Check results (2026-10-20): `cargo build -p loopflow --lib --bins` pass after realign; earlier compress run lib `ownership task_location` 29/29, integration `ownership wave_machine handoff` 18/18, clippy clean, `check_migrations.py` pass; `native_runner_*_foreground` hang without a terminal (CI).
