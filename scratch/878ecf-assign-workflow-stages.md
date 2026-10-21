# 878ecf: Assign Workflow stages to people and hand Tasks over without a service

Status: draft design (kickoff, 2026-10-21). Jack Heart filed the direction on
2026-10-21. Accepted: node names a person, arrival transfers, leaving transfers
on, handover button and command, no Loopflow service. The mechanisms below are
kickoff choices. Consequential ones are in `scratch/questions.md`. Approval of
this draft is still pending.

Beneficiary: two or more people in one repository, each with their own Machine
and agents. Example: Jack implements and Dustin reviews designs.

## Demo

`lf-world build workflow-handover /tmp/hand` builds two Homes, `jack` and
`dustin`. They share a bare `origin.git` and the shared planning ref. A world
`gh` stands in for GitHub, and a stub agent runs Flow steps. The repository
commits `.lf/workflows/review.yaml`:

```yaml
nodes:
  design: { skill: review-design, person: dustin@example.com, description: Dustin reviews the plan }
edges:
  - { from: start, to: design, flow: task-design }
  - { from: design, to: design, flow: task-design }
  - { from: design, to: end, flow: ship }
```

```bash
# Jack                                      # Dustin
lf task run w-1 review
# → task-design runs, arrives at design
# → "Handed to Dustin at design · pushed jack/w-1-gpu-fallback@3f2a91c"
                                            lf refresh
                                            lf task list --available
                                            # w-1  design  from Jack · 3f2a91c · "plan in scratch/"
                                            # Desktop (Dustin's LF_HOME): w-1 under Available
                                            lf session connect --task w-1
                                            # checkout at 3f2a91c; review-design guidance; claim on dustin-mbp
                                            lf task run ship -m "Approved; keep the CPU path"
                                            # → "Handed back to Jack: ship"
lf refresh
lf task status w-1   # owner Jack · Dustin chose ship at design: "Approved; keep the CPU path"
# Jack's live Task Session gets the same text as a steer
lf task run w-1      # runs ship, the edge Dustin chose
```

Desktop on Jack's side shows the Task as "With Dustin · design · not opened
yet". Once Dustin's claim lands it reads "With Dustin · opened on dustin-mbp",
and the Task returns to Available when Dustin hands it back.

## Problem, and what exists today

- A Workflow node (`ops/workflow.rs`, `WorkflowNode {name, skill, description}`)
  only marks where the owner takes part. `AuthoredNode` has
  `deny_unknown_fields` (`workflow.rs:97-110`), so `person:` fails to parse.
- Workflow position (`task_workflows`, `task_workflow_moves`) is local to one
  Machine and never exchanged (`planning_write.rs:804-920` asserts this).
- Ownership is an append-only log of (person, Machine) entries, `claim`,
  `handoff`, `unclaim` and `recorded` (`durable.rs:195-258`,
  `store/sqlite/task_ownership.rs`). Each entry is exchanged as a
  `<!-- loopflow-ownership:{json} -->` Task comment over Git or Linear.
  `Handoff` moves the claim between one person's Machines (`lf machine use
  --task`). Cross-person handoff is documented as deferred
  (`docs/architecture/machines.md:143`, `docs/lf.md:175`).
- Arriving at a node writes the position and nothing else. Nothing notifies
  anyone. "Available" is purely per-Session (`sessions.rs:available_sql`), and
  Desktop's Available filter only reads Sessions (`WorkModel.swift:1588`).
- The earlier "immutable creating-Machine coordinator" decision (product
  memory, LOO-427) was replaced by the ownership log in #1610
  (`0.14.12.001_release.sql` drops `task_admissions`). It does not block
  transfer.

## Chosen approach

**A handover is a `handoff` ownership entry that names another person.** It is
the same log, the same marker comment and the same exchange. One entry
generalizes the existing within-person handoff instead of adding a fourth
ownership concept. The entry carries what the receiver needs to continue:

```
TaskOwnershipEntry {
  id, task_id, kind: handoff, person, person_key,   // recipient (person_key from LOO-555)
  machine_id: Option,      // recipient Machine when known (handing back to the owner), else None
  handover: Option<Handover>,                        // present for cross-person handoff
  at,
}
Handover {
  node: String,            // Workflow node the Task waits at
  edge: Option<String>,    // the recipient's next edge, when handing back a decision
  returns_to: Option<PersonKey>, // who takes it back on leaving a person node; None for a manual handover
  head: Option<String>,    // pushed branch commit the receiver continues from
  graph: WorkflowDefinition, // captured graph, so the receiver takes up the same fixed graph
  note: Option<String>,    // handover note; also the comment's prose
}
```

The marker comment's prose is the human-readable record ("Jack Heart handed this
to Dustin at design: plan in scratch/"). Once LOO-555 makes Git comments steer,
that prose reaches the recipient's Sessions with no further work.

### Node person

`AuthoredNode` and `WorkflowNode` gain `person: Option<String>`, a person key
(Git `user.email`, lowercase; see questions). It is captured in the graph like
every other node field. Parse rejects a value with no `@`. Unknown people are
not an error: a key that has never been seen still receives the handover, and
the record waits on the shared ref.

### Transfer protocol

All writes go through the existing ownership writer in one IMMEDIATE
transaction, after the network step:

1. **Enter a person node.** This happens on arrival (`workflow_arrive`) or on
   `task workflow move` into a node whose `person` is not you. The current
   holder's Machine then:
   1. Pushes the Task branch. The push is the network step, before the
      transaction.
   2. Appends `handoff{person: node.person, handover{node, returns_to: you,
      head, graph, note}}`.
   3. Requests review on GitHub, best effort (see Reaching offline people).

   If the push fails, the arrival stands and no handoff is written. The Task is
   then *due for handover*. That state is derived: a person node naming someone
   else, still held by you. `lf task status` and Desktop show "Hand to Dustin"
   with the push error, and `lf task handover` retries.
2. **Receive.** Planning import ingests the marker (`ingest_marker_in`).
   - The recipient's Machine takes up `graph` at `node`, with a new move kind
     `received`, if its local Workflow row is absent or differs.
   - It sends a steer to that person's live Sessions for the Task. This is the
     same path as `notify_superseded_in`.
   - Nothing launches, and import creates no checkout.
3. **Open.** The recipient opens the Task through Desktop or `lf session connect
   --task`. Placement checks out the Task's recorded branch (LOO-555 item 2) at
   or after `head`, and claims on their Machine (`place_task` → `claim_in`). The
   claim is the acknowledgement the sender sees.
   - If `origin` lacks `head`, opening reports that Jack's push has not
     arrived. It does not silently start from an older commit.
4. **Leave a person node.** The holder runs `lf task run <edge>` or `lf task
   workflow move <node>` at a node naming them, and `returns_to` is set.
   - The command pushes their commits, if their checkout is ahead.
   - It appends `handoff{person: returns_to, machine_id: returns_to's last claim
     Machine, handover{node, edge, head, graph}}`.
   - Edges run on the owner's Machine: the reviewer decides, the owner's
     agents build.
   - An edge to `end` without a Flow completes in place on the reviewer's
     Machine, as `reach_end` does today.
5. **Take back.** The owner's Machine imports the handback. The owner is
   `returns_to` on the named Machine. A live Task Session gets a steer:
   "Dustin chose ship at design: …; run `lf -b task run ship`".
   - `lf task run` with no name, at that node, uses the handed-back `edge`. A
     named edge overrides it.
   - Before its Flow starts, the run fast-forwards a clean checkout to `head`.
     If the checkout has diverged, it reports and stops.
   - The next arrival at a person node repeats step 1, which covers both
     "next named person" and design loops.
6. **Manual handover.** `lf task handover <task> --to <person> [-m note]` hands
   the Task over with `returns_to: None`, so the recipient becomes its owner.
   Only the holder can hand over, as with `require_my_claim_in`. Uncommitted
   files do not travel. The command proceeds and appends "N uncommitted files
   stayed on <machine>" to the note rather than committing someone's work in
   progress.

Concurrency keeps today's rule: the latest entry in provider order wins. An
owner who reclaims while the Task is with Dustin supersedes Dustin's hold, and
Dustin's Sessions get the existing superseded steer. There is no lock.

### What travels, and what is lost without a service

| Travels | How |
|---|---|
| Code and committed design (`scratch/`) | push of the Task branch; `head` pins it |
| Task brief, comments, steers | planning (Git ref or Linear), as today |
| Workflow graph and node | `handover.graph/node` in the marker |
| Reviewer's decision | `handover.edge` + note |
| Ownership and acknowledgement | handoff entry, then the recipient's claim |

| Lost | Design around it |
|---|---|
| Push to someone whose Loopflow is not running | GitHub review request when a PR exists. Linear assignee under Linear planning. Otherwise it waits on the ref, and the sender sees "not opened yet". |
| Session history | Native history stays on the sender's Machine, as with Machine handoff. The note, committed scratch and comments carry intent. |
| Live presence | The recipient's claim is the only acknowledgement. Show its age. |
| Atomic transfer | Latest entry wins, and the superseded side is told by steer. |
| Running the next edge while the owner is away | None. An import never launches work, so the owner's run or live Session picks it up. |

### Reaching offline people

- **GitHub.** When the Task branch has an open PR, `gh pr edit --add-reviewer
  <login>` runs with the login resolved through `gh api
  repos/{o}/{r}/commits?author=<email>&per_page=1 --jq '.[0].author.login'`.
  This was probed on 2026-10-21 and returns `jacklionheart` for Jack's email.
  It fails quietly to "no GitHub login for dustin@…" when the person has never
  committed. No PR is opened just to notify.
- **Linear.** The existing `assert_assignee` path sets the assignee to the
  recipient's Linear user, looked up by email. Linear notifies them. Handing
  back reassigns the owner.

### Experience

- **CLI.**
  - `lf task handover <task> --to <email|known name> [-m note]`.
  - `lf task list --available` lists Tasks whose next action is yours: handed
    to you, or with an Available Session.
  - `lf task status` shows "With Dustin · design · not opened yet | opened on
    dustin-mbp 14:02", or the handed-back decision.
  - `lf task run` output names each handover it wrote.
  - A known name resolves to a key observed in this repository's ownership
    log. An ambiguous name is an error that lists the matches.
- **Desktop.**
  - Every Task workspace header has a **Hand over** button. It opens a popover
    with a person picker (people observed in the repository's planning, plus
    email entry), a note field, and the holder-only rule.
  - The Available sidebar filter includes Tasks handed to you that have no live
    Session.
  - A Task that is with someone else shows "With Dustin" and never counts as
    Available for you.
  - The Workflow graph marks person nodes with their person.
- **Agents.** Workflow guidance (`Workflow::guidance`) names the node's person.
  At a node naming someone else, it says "handed to Dustin; leave the branch
  alone" in place of edge commands.

## Source of truth and affected surfaces

- The ownership log is authoritative for who holds a Task, and is exchanged.
  The holder, due-handover state and pending decision are derived from it
  together with the local Workflow row.
- `task_workflows` stays local. On the receiving side it is seeded from
  `handover.graph`, and it is never exchanged as its own record.
- Schema: one migration draft. `task_ownership` gains `handover TEXT` (JSON
  `Handover`, NULL otherwise). `task_workflow_moves.kind` allows `received`.
- Wire DTOs:
  - `TaskOwnershipEntry` adds `handover`.
  - The Task runtime owner gains `is_you: bool` and
    `handover: Option<TaskHandoverView {from, node, edge, note, head, at,
    opened_on}>`.
  - The Workflow node DTO adds `person`.
  - Fixtures go under `tests/fixtures/dto/`, with Swift mirrors in
    `swift/Loopflow/Models/TaskWork.swift` and the owner model.
- Docs to update: `docs/lf.md` "Work with another person" and the ownership
  table, `docs/architecture/machines.md` (cross-person handoff is no longer
  deferred), workflow authoring docs (`person:`), and the regenerated
  `docs/lf-reference.md`.

## Alternatives considered

- **Exchange Workflow position as a planning record.** Every Machine would see
  every move. That doubles the authority over position and crosses "planning
  exchange excludes execution". Rejected: only handovers need the graph.
- **The reviewer runs the next edge on their own Machine.** That is the same as
  takeover: Dustin's agents would build on Jack's branch. It stays available as
  a manual handover. It is rejected as the default because the request says the
  Task returns to Jack.
- **Auto-run the handed-back edge on import.** This would bring "auto
  transfer" closer, but it breaks "imports create no requests" and launches
  work on a Machine nobody is watching. Kept as an open question.
- **List handed Tasks in `lf session list --available`.** The handed Task has
  no Session on Dustin's Machine. Mixing Task rows into the Session DTO would
  make two shapes in one list. `lf task list --available` was chosen instead
  (see questions).

## Wild failure, pre-empted

- A handover silently strands work. The due-handover state is derived, so a
  failed push shows a button rather than a lost Task.
- Two branches or two PRs result. This depends on LOO-555 item 2, where the
  recorded branch is reused by every holder.
- Dustin's operator claims the returned Task. This depends on LOO-553: an
  operator leaves others' Tasks alone.
- An email typo in `person:` hands work into the void. `lf task status` shows
  "not opened yet" with the age, and the holder can take it back with
  `lf task claim`.

## Exclusions

- Approval semantics beyond choosing an edge.
- Multiple people per node.
- Teams or groups.
- Session history transfer.
- Opening a PR to notify.
- Short-name `person:` values in workflow files.
- Mobile.

## Dependencies

This lands after **LOO-555** (person key, branch reuse, Git comment steers) and
**LOO-552** (shared ref by default; the demo world can configure `shared:`
explicitly until then). LOO-555 drafts the person key as the random
`planning_user` UUID. This design needs an email-like key that a committed
workflow file can name. See questions.

## Delete: do not maintain

- The "cross-person handoff is deferred" text in `docs/architecture/machines.md`
  and `docs/lf.md`, and the matching line in `docs/architecture-reference.md`.
- The `Handoff` doc comment "keeping the person" (`durable.rs`). Handoff now
  names its recipient.

Forbidden:
- A separate transfer table or record type alongside the ownership log.
- Exchanging `task_workflows`.
- Launching from import.
- A second "available" judgment in Swift. Rust emits `is_you` and
  `handover`, and Swift only filters.

## Slices (one PR, after LOO-555)

1. Node `person` in the definition, the graph DTO and validation. Focused test:
   `cargo test -p loopflow --lib workflow::`.
2. The `handover` column and marker. Ingest seeds the Workflow and steers.
   Arrival and move write the handoff. Run uses the handed-back edge and fast-
   forwards. `lf task handover`. `task list --available`.
3. GitHub reviewer and Linear assignee notification.
4. Desktop: Hand over popover, Available inclusion, "With Dustin" state, and
   person on graph nodes.
5. The `workflow-handover` lf-world recipe, the acceptance suite and docs.

This slice: 1.

## Done when (gate)

- `cargo test -p loopflow --test integration workflow_handover`: in one built
  two-Home world, the demo script holds end to end.
  - Dustin's `task list --available --json` contains w-1 with `node: design`
    and Jack's `head`.
  - Dustin's checkout HEAD equals `head`.
  - After the handback, Jack's `task status --json` owner `is_you`, with the
    handover `edge: ship`.
  - Jack's bare `task run` starts `ship`.
  - A failed push leaves Jack's Task due for handover, with no handoff entry.
  - The world `gh` records a reviewer request for Dustin's login when a PR
    exists.
- `swift test --package-path swift --filter WorkModelTests`: a Task handed to
  you with no Session appears under Available. A Task held by someone else does
  not.
- `cargo test -p loopflow --test dto_fixtures` and the Swift fixture tests
  round-trip the new fields.
- Review and demo: Jack and Dustin run the demo on the lf-world with Desktop
  open on both Homes.

Check: kickoff only; no build or tests run (prose).
