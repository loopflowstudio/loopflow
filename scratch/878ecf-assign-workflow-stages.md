# 878ecf: Assign Workflow stages to people and hand Tasks over without a service

Status: draft design. Kickoff was 2026-10-21, revised the same day after Jack
Heart's design review. Jack Heart filed the direction on 2026-10-21.

- **Accepted (brief):** a node names a person, arrival transfers, leaving
  transfers on, a handover button and command, and no Loopflow service.
- **Accepted (Jack, design review 2026-10-21):**
  - A handover addresses a person (their provider account and their
    default-pickup Machine) or a Machine (public, or one of theirs).
  - Loopflow keeps Person and Machine records in Git as the backup where
    providers fall short.
  - Design to the Linear and Asana core; Notion may go.
  - No GitHub review requests.
  - LOO-555 lands the email person key and `refs/loopflow/machines/<id>` first.
- Everything else here is a kickoff mechanism choice. Open items are in
  `scratch/questions.md`. Approval of this draft is still pending.

Beneficiary: two or more people in one repository, each with their own Machine
and agents. Example: Jack implements and Dustin reviews designs.

## Demo

`lf-world build workflow-handover /tmp/hand` builds two Homes, `jack` and
`dustin`. They share a bare `origin.git` and the shared planning ref. A stub
agent runs Flow steps. Each Home publishes its Machine record on first sync.
Dustin's laptop is his pickup Machine. The repository commits
`.lf/workflows/review.yaml`:

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
# → "Handed to Dustin (dustin-mbp) at design · pushed jack/w-1-gpu-fallback@3f2a91c"
                                            lf refresh
                                            lf task list --available
                                            # w-1  design  from Jack · 3f2a91c · "plan in scratch/"
                                            # Desktop (Dustin's LF_HOME): w-1 under Available
                                            lf session connect --task w-1
                                            # checkout at 3f2a91c; review-design guidance; claim on dustin-mbp
                                            lf task run ship -m "Approved; keep the CPU path"
                                            # → "Handed back to Jack (jack-mbp): ship"
lf refresh
lf task status w-1   # owner Jack · Dustin chose ship at design: "Approved; keep the CPU path"
# Jack's live Task Session gets the same text as a steer
lf task run w-1      # runs ship, the edge Dustin chose
```

Desktop on Jack's side shows the Task as "With Dustin · design · not opened
yet". Once Dustin's claim lands it reads "With Dustin · opened on dustin-mbp".
The Task returns to Available when Dustin hands it back. A second world variant
hands to the public Machine `studio-mini` and checks that only that Machine
lists it.

## Problem, and what exists today

- A Workflow node (`ops/workflow.rs`, `WorkflowNode {name, skill, description}`)
  only marks where the owner takes part. `AuthoredNode` has
  `deny_unknown_fields` (`workflow.rs:97-110`), so `person:` fails to parse.
- Workflow position (`task_workflows`, `task_workflow_moves`) is local to one
  Machine and never exchanged (`planning_write.rs:804-920` asserts this).
- Ownership is an append-only log of entries: `claim`, `handoff`, `unclaim` and
  `recorded` (`durable.rs:195-258`, `store/sqlite/task_ownership.rs`).
  - Each entry is (person, Machine) and travels as a
    `<!-- loopflow-ownership:{json} -->` Task comment over Git or Linear.
  - `Handoff` moves a claim between one person's Machines (`lf machine use
    --task`).
  - Cross-person handoff is documented as deferred (`docs/architecture/machines.md:143`).
- A person is a display-name string (`participant_name()`). Machines are local
  rows and are never exchanged. Nothing can address another person's Machine.
  Research: `scratch/878ecf-people-research.md`.
- Under Linear, the assignee is only ever set to this Machine's own viewer
  (`ops/task_ownership.rs:70 assert_assignee`).
- Arriving at a node writes the position and nothing else, so nobody is
  notified. "Available" is per Session (`sessions.rs:available_sql`), and
  Desktop's Available filter reads only Sessions (`WorkModel.swift:1588`).

## Chosen approach

### People and Machines in Git

LOO-555 lands the base. The person key is the lowercase Git `user.email`.
Each Machine force-pushes its single-writer record to
`refs/loopflow/machines/<machine-id>`, which carries
`{machine_id, label, person_key, person, crons}`. The ref sits in the
repository's `origin` under every planning transport, so it works the same with
Linear and with the Git ref.

878ecf adds three fields to that record:

```
MachineRecord {
  machine_id, label, person_key, person, crons,   // LOO-555
  pickup: bool,      // "I pick up work handed to my person" (e.g. the laptop)
  public: bool,      // other people may hand Tasks to this Machine
  accounts: Vec<ProviderAccount>,                 // this Machine's provider identities
  updated_at,
}
ProviderAccount { provider: linear | asana, id, email }
```

There is no Person record and no Person ref. A Person is derived by grouping
Machine records by `person_key`: name, Machines, pickup Machine and provider
accounts. Every Machine already knows its own provider identity (the Linear
viewer, the Asana `/users/me`), so the writer that holds a fact is the one that
publishes it. If two Machines of one person both set `pickup`, the later
`updated_at` wins.

Teams are excluded. Linear and Asana have teams, but no consumer here needs a
team yet (one person per node).

Commands:
- `lf machine pickup` marks this Machine as the person's pickup Machine.
- `lf machine public on|off` sets whether others may hand Tasks to it.
- `lf machine list --all` lists teammates' Machines from the fetched records,
  with person, pickup and public.
- `lf person list` lists the derived people: key, name, pickup Machine and
  accounts. Desktop's picker reads the same list.

### Address

```
HandoverAddress = Person(PersonKey) | Machine(MachineLabel or MachineId)
```

The sender resolves the address against the fetched records when it writes the
entry:
- **Person.** `person_key` is the recipient's key. `machine_id` is left `None`,
  so pickup is resolved when the Task is read. Their provider account receives
  the assignment.
- **Machine.** The Machine must be public, or belong to the sender. It
  resolves to `(machine.person_key, machine_id)`.
  - A Machine of your own is the existing same-person handoff. `lf task
    handover --to <your mini>` and `lf machine use mini --task` share one
    implementation.
  - An unknown or ambiguous label is an error that lists the matching Machines.

The Task is *picked up here* when either:
- the entry's `machine_id` is this Machine, or
- `machine_id` is `None`, the entry's `person_key` is yours, and this Machine is
  your pickup Machine. If you have published no pickup Machine, every one of
  your Machines qualifies.

Resolving at read time means a pickup Machine Dustin sets later still receives
a handover already waiting on the ref.

### Handover entry

A handover is a `handoff` ownership entry naming another person or Machine. It
is the same log, the same marker comment and the same exchange: one entry
generalizes the existing within-person handoff, and no fourth ownership concept
is added.

```
TaskOwnershipEntry {
  id, task_id, kind: handoff, person, person_key,   // recipient (person_key from LOO-555)
  machine_id: Option,      // addressed Machine, or the owner's Machine when handing back; None = their pickup
  handover: Option<Handover>,                        // present for a cross-person handoff
  at,
}
Handover {
  node: String,            // Workflow node the Task waits at
  edge: Option<String>,    // the recipient's next edge, when handing back a decision
  returns_to: Option<(PersonKey, MachineId)>, // who takes it back on leaving a person node; None for a manual handover
  head: Option<String>,    // pushed branch commit the receiver continues from
  graph: WorkflowDefinition, // captured graph, so the receiver takes up the same fixed graph
  note: Option<String>,    // handover note; also the comment's prose
}
```

The marker comment's prose is the human-readable record ("Jack Heart handed this
to Dustin at design: plan in scratch/"). Once LOO-555 makes Git comments steer,
that prose reaches the recipient's Sessions with no further work.

### Node person

`AuthoredNode` and `WorkflowNode` gain either `person: <email>` or
`machine: <label>`, but not both. The field is captured in the graph like every
other node field.
- Parse rejects a `person` value with no `@`.
- An unknown person is not an error. The handover is written, waits on the ref,
  and shows "not opened yet".
- An unknown `machine` cannot be resolved. The arrival stands, and the Task is
  *due for handover* with that error.

### Transfer protocol

All writes go through the existing ownership writer in one IMMEDIATE
transaction, after the network step.

1. **Enter a person node.** This happens on arrival (`workflow_arrive`) or on
   `task workflow move` into a node that addresses someone other than you. The
   holder's Machine:
   1. Pushes the Task branch. The push is the network step.
   2. Appends `handoff{to, handover{node, returns_to: (you, this Machine),
      head, graph, note}}`.
   3. Under Linear planning, sets the issue assignee to the recipient's Linear
      account. Under Asana, it sets the Asana assignee (lands with the Asana
      provider, LOO-520). The provider sends the notification.

   If the push fails, the arrival stands and no handoff is written. The Task is
   then *due for handover*, a derived state: the node addresses someone else
   and you still hold the Task. `lf task status` and Desktop show "Hand to
   Dustin" with the error, and `lf task handover` retries.
2. **Receive.** Planning import ingests the marker (`ingest_marker_in`). On the
   Machine that picks it up:
   - It takes up `graph` at `node`, with a new move kind `received`, if its
     local Workflow row is absent or differs.
   - It sends a steer to that person's live Sessions for the Task, on the same
     path as `notify_superseded_in`.
   - Nothing launches, and import creates no checkout.
3. **Open.** The recipient opens the Task through Desktop or `lf session connect
   --task`. Placement checks out the Task's recorded branch (LOO-555 item 2) at
   or after `head`, then claims on their Machine (`place_task` → `claim_in`).
   The claim is the acknowledgement the sender sees.
   - If `origin` lacks `head`, opening reports that the push has not arrived.
     It never starts from an older commit.
4. **Leave a person node.** The holder runs `lf task run <edge>` or `lf task
   workflow move <node>` at a node addressing them, and `returns_to` is set.
   - The command pushes their commits if their checkout is ahead.
   - It appends `handoff{to: returns_to, handover{node, edge, head, graph}}`.
   - It reassigns the provider assignee to the owner.
   - Edges run on the owner's Machine: the reviewer decides, and the owner's
     agents build.
   - An edge to `end` without a Flow completes in place, as `reach_end` does
     today.
5. **Take back.** The owner's Machine imports the handback. A live Task
   Session gets a steer: "Dustin chose ship at design: …; run `lf -b task run
   ship`".
   - A bare `lf task run` at that node uses the handed-back `edge`. A named
     edge overrides it.
   - Before its Flow starts, the run fast-forwards a clean checkout to `head`.
     If the checkout has diverged, the run reports that and stops.
   - The next arrival at a person node repeats step 1, which covers both "next
     named person" and design loops.
6. **Manual handover.** `lf task handover <task> --to <email|name|machine>
   [-m note]` writes the handoff with `returns_to: None`, so the recipient
   becomes the owner. Only the holder can hand over (`require_my_claim_in`).
   Uncommitted files do not travel. The command proceeds and appends "N
   uncommitted files stayed on <machine>" to the note rather than committing
   someone's work in progress.
7. **Reassignment in the provider.** Jack Heart (2026-10-21): reassigning in
   Linear gives the Task to the new person without stopping the existing
   holder's work.
   - Linear observe sees the assignee change to an account that is not the
     holder's. It resolves the account to a person key through Machine
     records' `accounts`, falling back to the Linear user's email.
   - It appends `handoff{person, handover{node, returns_to: None, head: None,
     note: "<actor> reassigned in Linear"}}`. The entry ID derives from the
     Linear history event, so every Machine that observes it writes the same
     entry once.
   - The previous holder keeps running: live Flows and Sessions continue,
     nothing is cancelled or unclaimed. Their Sessions get a steer, and the
     ledger shows "Dustin took this in Linear". What they do with their
     in-flight work is their call.
   - The recipient opens the Task at whatever its recorded branch has on
     `origin`, since nobody pushed for this handover.
   - Loopflow's own assignee writes match the holder and produce no entry.

Concurrency keeps today's rule: the latest entry in provider order wins. An
owner who reclaims while the Task is with Dustin supersedes Dustin's hold, and
Dustin's Sessions get the existing superseded steer. There is no lock.

`assert_assignee` reshapes from "this Machine's viewer while claimed here" to
"the current holder's provider account". The Machine that writes an entry sets
the assignee, so a Machine never needs a credential for anyone else.

### What travels, and what is lost without a service

| Travels | How |
|---|---|
| Code and committed design (`scratch/`) | push of the Task branch; `head` pins it |
| Task brief, comments, steers | planning (Git ref or Linear), as today |
| Workflow graph and node | `handover.graph/node` in the marker |
| Reviewer's decision | `handover.edge` and the note |
| Ownership and acknowledgement | the handoff entry, then the recipient's claim |
| Who and where to deliver | Machine records on `refs/loopflow/machines/*` |

| Lost | Design around it |
|---|---|
| Push to someone whose Loopflow is not running | Provider assignment (Linear or Asana notifies). Under Git-only planning nothing notifies; the handover waits on the ref, and the sender sees "not opened yet" with its age. |
| Session history | Native history stays on the sender's Machine, as with Machine handoff. The note, committed scratch and comments carry intent. |
| Live presence | The recipient's claim is the only acknowledgement. Show its age. Machine records are as fresh as the last sync. |
| Atomic transfer | The latest entry wins, and the superseded side is told by steer. |
| Running the next edge while the owner is away | None. An import never launches work. |

### Experience

- **CLI.**
  - `lf task handover <task> --to <email|name|machine> [-m note]`. A name
    resolves against `lf person list` and must match exactly one person. An
    ambiguous name is an error that lists the matches.
  - `lf task list --available` lists Tasks whose next action is yours on this
    Machine: picked up here, or with an Available Session.
  - `lf task status` shows "With Dustin · design · not opened yet" or "With
    Dustin · opened on dustin-mbp 14:02", or the handed-back decision.
  - `lf task run` output names each handover it wrote and the resolved Machine.
- **Desktop.**
  - Every Task workspace header has a **Hand over** button. Its popover
    offers:
    - a picker of people and of public Machines (plus email entry), built from
      `lf person list` and the Machine records;
    - a note field;
    - the holder-only rule.
  - The Available sidebar filter includes Tasks picked up here that have no
    live Session.
  - A Task held by someone else shows "With Dustin" and never counts as
    Available for you.
  - The Workflow graph marks each person node with its person or Machine.
- **Agents.** Workflow guidance (`Workflow::guidance`) names the node's
  addressee. At a node addressing someone else, it says "handed to Dustin;
  leave the branch alone" in place of edge commands.

## Source of truth and affected surfaces

- **Who holds a Task:** the ownership log, which is exchanged. The holder,
  due-handover state, pickup and pending decision are derived from it, the
  Machine records and the local Workflow row.
- **Who people are and where they pick up:** Machine records on
  `refs/loopflow/machines/*`, one writer each. Locally they are cached in the
  `machines` table (LOO-555) with the new columns.
- `task_workflows` stays local. On the receiving side it is seeded from
  `handover.graph`, and it is never exchanged as its own record.
- **Schema:** one migration draft.
  - `task_ownership` gains `handover TEXT` (JSON `Handover`, NULL otherwise).
  - `task_workflow_moves.kind` allows `received`.
  - The Machine record cache gains `pickup`, `public`, `accounts` and
    `updated_at`.
- **Wire DTOs:**
  - `TaskOwnershipEntry` adds `handover`.
  - The Task runtime owner gains `is_you: bool` and
    `handover: Option<TaskHandoverView {from, to, node, edge, note, head, at,
    opened_on}>`.
  - The Workflow node DTO adds `person` and `machine`.
  - A new `PersonView {key, name, machines, pickup, accounts}` is emitted by
    `lf person list --json`.
  - Fixtures go under `tests/fixtures/dto/`, with Swift mirrors in
    `swift/Loopflow/Models/TaskWork.swift` and the owner model.
- **Docs:** update `docs/lf.md` "Work with another person" and its ownership
  table, `docs/architecture/machines.md` (cross-person handoff is no longer
  deferred; publication, pickup and public), workflow authoring (`person:` and
  `machine:`), and regenerate `docs/lf-reference.md`.

## Alternatives considered

- **Exchange Workflow position as a planning record.** Rejected. It doubles the
  authority over position and crosses "planning exchange excludes execution".
  Only handovers need the graph.
- **A separate Person ref.** Rejected. Every fact a Person record would hold
  (name, provider accounts, pickup) is known by one of that person's Machines,
  and a shared person ref would have several writers needing merge logic.
- **Provider users as the only roster.** Rejected. This was Jack's direction:
  no provider can hold a Machine, pickup or public flag, and Git-only planning
  has no users at all. Provider accounts appear as fields on a record, never as
  the source of who exists.
- **Keep the Linear assignee one-way.** Rejected by Jack (question 7): a
  reassignment in Linear is a handover. The log stays authoritative because
  observation writes the reassignment into it.
- **Reviewer runs the next edge on their own Machine.** That is a takeover. It
  stays available as a manual handover.
- **Auto-run the handed-back edge on import.** This breaks "imports create no
  requests". Kept as an open question.
- **GitHub review requests.** Dropped by Jack on 2026-10-21. Notification is
  provider assignment, or nothing.

## Wild failure, pre-empted

- **A handover silently strands work.** Due-for-handover is derived, so a
  failed push shows a button rather than losing the Task.
- **A handover lands on the wrong Machine.** The pickup Machine is resolved at
  read time and shown on the sender's side: "With Dustin (dustin-mbp)".
- **A stale Machine record routes to a dead Machine.** `lf machine list --all`
  shows each record's age. The sender can re-address with `lf task handover`,
  and the holder can take it back with `lf task claim`.
- **Two branches or two PRs.** Depends on LOO-555 item 2: every holder reuses
  the recorded branch.
- **Dustin's operator claims the returned Task.** Depends on LOO-553: operators
  leave others' Tasks alone.
- **An email typo in `person:`.** The handover shows "not opened yet" with its
  age, and the holder can reclaim it.

## Exclusions

- Approval semantics beyond choosing an edge.
- Multiple people per node, and Teams.
- Session history transfer.
- GitHub notification.
- Short names in workflow files.
- Notion.
- Mobile.

## Dependencies

- **LOO-555** (in its autonomous Flow at `implement`, 2026-10-21) must land
  first with:
  - the email person key;
  - `refs/loopflow/machines/<machine-id>`;
  - branch reuse;
  - Git comment steers.

  Its committed design still reads `planning_user` UUID and
  `refs/loopflow/crons/<machine-id>`, because Jack's steer arrived mid-Flow. If
  it lands that way, 878ecf's first slice converts the key and renames the ref
  in place, with no dual reading (questions 1 and 8).
- **LOO-552** makes the shared ref the default. Until then the demo world
  configures `shared:` explicitly.
- **Asana assignee** waits for the Asana provider (LOO-520). Until then 878ecf
  ships only the `ProviderAccount` shape and Linear behavior.

## Delete: do not maintain

- The "cross-person handoff is deferred" and "Nothing routes to another
  person's Machine" text in `docs/architecture/machines.md` and `docs/lf.md`,
  and the matching line in `docs/architecture-reference.md`.
- The `Handoff` doc comment "keeping the person" (`durable.rs`).
- `assert_assignee`'s viewer-only rule (`ops/task_ownership.rs:66-96`). The
  holder-account rule replaces it.
- The separate same-person handoff path behind `lf machine use --task`, merged
  into the one handover writer. The command stays.

Forbidden:
- A separate transfer table or record type alongside the ownership log.
- A Person ref or users table beside the Machine records.
- Exchanging `task_workflows`.
- Launching from import.
- A second "available" or pickup judgment in Swift. Rust emits `is_you`,
  `handover` and pickup, and Swift only filters.
- Reading both `refs/loopflow/crons/*` and `refs/loopflow/machines/*`.

## Slices (one PR, after LOO-555)

1. Node `person` and `machine` in the definition, the graph DTO and validation.
   Focused test: `cargo test -p loopflow --lib workflow::`.
2. Machine record fields, `lf machine pickup` and `lf machine public`,
   `lf machine list --all` and `lf person list`, and pickup resolution.
3. The `handover` column and marker. Ingest seeds the Workflow and steers.
   Arrival and move write the handoff. Run uses the handed-back edge and
   fast-forwards. Also `lf task handover` and `task list --available`.
4. Holder-account assignee under Linear.
5. Desktop: Hand over popover, Available inclusion, "With Dustin" state, and
   the addressee on graph nodes.
6. The `workflow-handover` lf-world recipe, the acceptance suite and docs.

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
  - In a third Home that is Dustin's second, non-pickup Machine, the Task is not
    listed as available.
  - Handing to public `studio-mini` lists it on that Machine only.
  - Handing to a non-public Machine of someone else is refused, with the
    public Machines listed.
- `cargo test -p loopflow --lib task_ownership`: under a Linear test double, a
  handoff queues the recipient's Linear account as assignee, and a handback
  queues the owner's. A reassignment observed in Linear appends exactly one
  handoff to the new person across two observing Machines, and the previous
  holder's running Flow keeps running with a steer.
- `swift test --package-path swift --filter WorkModelTests`: a Task picked up
  here with no Session appears under Available, and a Task held by someone else
  does not.
- `cargo test -p loopflow --test dto_fixtures` and the Swift fixture tests
  round-trip the new fields and `PersonView`.
- Review and demo: Jack and Dustin run the demo on the lf-world with Desktop
  open on both Homes.

Check: kickoff revision only; no build or tests run (prose).
