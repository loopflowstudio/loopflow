# 878ecf: Assign Workflow stages to people and hand Tasks over without a service

Status: draft design. Kickoff was 2026-10-21, revised three times the same day after
Jack Heart's design review and his person-only direction. Jack Heart filed the direction on 2026-10-21.

- **Accepted (brief):** a node names a person, arrival transfers, leaving
  transfers on, a handover button and command, and no Loopflow service.
- **Accepted (Jack, design review 2026-10-21):**
  - A handover addresses a person (their provider account), or one of your
    own Machines. Another person's Machine is never addressed.
  - Only the Person reaches Git: key = lowercase email, name, provider
    accounts. Machines are never published. Pickup is a local per-Machine
    setting ("this Machine picks up work addressed to me"). This supersedes
    the earlier Machine-record design (Jack, later on 2026-10-21).
  - Git is the backup where providers fall short; design to the Linear and
    Asana core; Notion may go.
  - No GitHub review requests.
  - A Linear reassignment is a person-level claim (a take), not a handoff.
  - LOO-555 lands the email person key first.
- Everything else here is a kickoff mechanism choice. Open items are in
  `scratch/questions.md`. Approval of this draft is still pending.

Beneficiary: two or more people in one repository, each with their own Machine
and agents. Example: Jack implements and Dustin reviews designs.

## Demo

`lf-world build workflow-handover /tmp/hand` builds Homes `jack`, `dustin`
and `dustin-mini` (Dustin's second Machine, `lf machine pickup off`). They
share a bare `origin.git` and the shared planning ref. A stub agent runs Flow
steps. Each Home publishes its Person record on first sync. The repository
commits
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
# → "Handed to Dustin at design · pushed jack/w-1-gpu-fallback@3f2a91c"
                                            lf refresh
                                            lf task list --available
                                            # w-1  design  from Jack · 3f2a91c · "plan in scratch/"
                                            # Desktop (Dustin's LF_HOME): w-1 under Available
                                            lf session connect --task w-1
                                            # checkout at 3f2a91c; review-design guidance; claims here
                                            lf task run ship -m "Approved; keep the CPU path"
                                            # → "Handed back to Jack: ship"
lf refresh
lf task status w-1   # owner Jack · Dustin chose ship at design: "Approved; keep the CPU path"
# Jack's live Task Session gets the same text as a steer
lf task run w-1      # runs ship, the edge Dustin chose
```

Desktop on Jack's side shows the Task as "With Dustin · design · not opened
yet". Once Dustin's claim lands it reads "With Dustin · opened 14:02".
`dustin-mini` never lists the Task.
The Task returns to Available when Dustin hands it back.

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
  rows and are never exchanged, and stay that way.
  Research: `scratch/878ecf-people-research.md`.
- Under Linear, the assignee is only ever set to this Machine's own viewer
  (`ops/task_ownership.rs:70 assert_assignee`).
- Arriving at a node writes the position and nothing else, so nobody is
  notified. "Available" is per Session (`sessions.rs:available_sql`), and
  Desktop's Available filter reads only Sessions (`WorkModel.swift:1588`).

## Chosen approach

### People in Git; Machines stay local

Jack Heart (2026-10-21): "the only thing that reaches git is the person, and
then you can mark machines to pick up things that match 'you'".

The person key is the lowercase Git `user.email` (LOO-555). A Person record
is the only identity Loopflow publishes:

```
PersonRecord {
  key,                              // lowercase email
  name,                             // display only
  accounts: Vec<ProviderAccount>,   // provider identities that are not derivable from the email
  updated_at,
}
ProviderAccount { provider: linear | asana, id, email }
```

- **Where.** `refs/loopflow/people/<key>` on `origin`, one ref per person,
  independent of the planning transport. The ref holds one `person.json`.
  Lowercase emails are valid ref components; a key Git rejects (`..`, a
  leading `.`, `.lock`) is an error at publish naming the email.
- **Writers.** Any Machine of that person, only when its content would
  change. Publish fetches the ref, merges (this Machine's account wins for
  its own provider; other providers' accounts are kept), and pushes with
  `--force-with-lease` against the fetched object. A lost lease refetches and
  retries once, then reports. Sync publishes after provider auth, so a
  Machine's Linear viewer reaches the record without a command.
- **Readers.** `lf refresh` and planning sync fetch `refs/loopflow/people/*`
  into a local `people` cache. `lf person list` and Desktop's picker read it.
- **Providers first.** Under Linear (and Asana with LOO-520), resolving an
  email to an account tries the record's `accounts`, then the provider's
  by-email lookup (`users(filter:{email})`, `GET /users/{email}`). Git-only
  planning has the records alone.

Machines are never published as records. There is no `refs/loopflow/machines/*`, no
`lf machine list --all`, and no Person derived from Machines. The one place a
Machine appears in Git is a cron its person chose to publish (`lf cron
publish`, LOO-555), which carries that Machine's key and label (Jack Heart,
2026-10-21). Nothing in 878ecf reads them. A teammate's
view of a Task names a person, never a Machine label. Ownership entries keep
their opaque `machine_id` (today's exchange), which only your own Machines can
resolve to a label.

**Pickup** is a local setting on each Machine: "this Machine picks up work
addressed to me". It defaults on, so a single laptop needs no setup. `lf
machine pickup off` on a cron host or a Mac mini keeps person-addressed Tasks
off its Available list. It is a column on the local `machines` row for this
Machine and never leaves it.

**Public Machines** (Jack Heart, 2026-10-21, questions 11 and 13). A Machine
has a private label (its local `machines.label`, never published) and an
optional public name. `lf machine public <name>` / `lf machine public off` is a
local setting, default off.
- Ownership entries written by a public Machine carry `machine_label` set to
  its public name, so teammates read "opened on studio-mini". A private
  Machine's entries carry no label, and teammates see the person only.
- Entries always keep the opaque `machine_id`, which your own Machines need
  to agree which one holds the Task.
- Public is visibility only. It does not make a Machine addressable:
  handovers still name a person.
- Published crons (LOO-555) carry the Machine key, and the public name when
  the Machine is public.

Teams are excluded: no consumer needs one yet (one person per node).

Commands:
- `lf person list` lists the fetched people: key, name and accounts.
- `lf machine pickup [on|off]` reads or sets this Machine's pickup.

### Address

```
HandoverAddress = Person(PersonKey) | OwnMachine(MachineId)
```

- **Person.** `person_key` is the recipient's key and `machine_id` is `None`.
  Their provider account receives the assignment.
- **Your own Machine.** Resolved from your local `machines` table. This is
  the existing same-person handoff: `lf task handover --to <your mini>` and
  `lf machine use mini --task` share one implementation. Another person's
  Machine cannot be named, because none is known here; to reach that person,
  name the person.
  - An unknown label is an error that lists your Machines and suggests
    `--to <email>`.

The Task is *picked up here* when either:
- the entry's `machine_id` is this Machine, or
- `machine_id` is `None`, the entry's `person_key` is yours, and pickup is on
  here.

Several of your Machines may list the same Task. The first that opens it
claims it; the others then read it as yours on that Machine (today's
same-person rule) and stop listing it as available.

### Handover entry

A handover is a `handoff` ownership entry naming another person or Machine. It
is the same log, the same marker comment and the same exchange: one entry
generalizes the existing within-person handoff, and no fourth ownership concept
is added.

```
TaskOwnershipEntry {
  id, task_id, kind: handoff, person, person_key,   // recipient (person_key from LOO-555)
  machine_id: Option,      // one of the sender's own Machines, or the owner's Machine on handback; None = the person's pickup Machines
  handover: Option<Handover>,                        // present for a cross-person handoff
  at,
}
Handover {
  node: String,            // Workflow node the Task waits at
  edge: Option<String>,    // the recipient's next edge, when handing back a decision
  returns_to: Option<(PersonKey, MachineId)>, // owner and their (opaque) Machine to return to; None for a manual handover
  head: Option<String>,    // pushed branch commit the receiver continues from
  graph: WorkflowDefinition, // captured graph, so the receiver takes up the same fixed graph
  note: Option<String>,    // handover note; also the comment's prose
}
```

The marker comment's prose is the human-readable record ("Jack Heart handed this
to Dustin at design: plan in scratch/"). Once LOO-555 makes Git comments steer,
that prose reaches the recipient's Sessions with no further work.

### Node person

`AuthoredNode` and `WorkflowNode` gain `person: <email>`. The field is
captured in the graph like every other node field. There is no `machine:` node
field: Machines are never published.
- Parse rejects a `person` value with no `@`.
- An unknown person is not an error. The handover is written, waits on the ref,
  and shows "not opened yet".

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
   holder's work. It is a take, not a handoff: a handoff is written by the
   holder's Machine actively relinquishing, and here nobody relinquished.
   - Linear observe sees the assignee change to an account that is not the
     holder's. It resolves the account to a person key through Person
     records' `accounts`, falling back to the Linear user's email.
   - It appends `claim{person, machine_id: None}` with no `handover`. A
     person-level claim, like a person handover, is picked up by that
     person's Machines with pickup on. The entry ID derives from the Linear history
     event, so every Machine that observes it writes the same entry once.
   - The previous holder is superseded, exactly as when an owner reclaims:
     live Flows and Sessions continue, nothing is cancelled or unclaimed, and
     their Sessions get the existing superseded steer. The ledger reads
     "Dustin took this in Linear". What happens to their in-flight work is
     their call.
   - Nothing was pushed for it, so the recipient opens the Task at whatever
     its recorded branch has on `origin`.
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
| Who to deliver to | Person records on `refs/loopflow/people/*`; pickup is local |

| Lost | Design around it |
|---|---|
| Push to someone whose Loopflow is not running | Provider assignment (Linear or Asana notifies). Under Git-only planning nothing notifies; the handover waits on the ref, and the sender sees "not opened yet" with its age. |
| Session history | Native history stays on the sender's Machine, as with Machine handoff. The note, committed scratch and comments carry intent. |
| Live presence | The recipient's claim is the only acknowledgement. Show its age. |
| Where the recipient will open it | Unknown to the sender by design: Machines are private. The sender sees the person and the claim time. |
| Atomic transfer | The latest entry wins, and the superseded side is told by steer. |
| Running the next edge while the owner is away | None. An import never launches work. |

### Experience

- **CLI.**
  - `lf task handover <task> --to <email|name|your-machine> [-m note]`. An
    email is always accepted, known or not. A name resolves against `lf
    person list` and must match exactly one person; an ambiguous name lists
    the matches. A label resolves only against your own Machines.
  - `lf task list --available` lists Tasks whose next action is yours on this
    Machine: picked up here, or with an Available Session.
  - `lf task status` shows "With Dustin · design · not opened yet" or "With
    Dustin · opened 14:02", or the handed-back decision. Your own Machine's
    label appears only when you hold it.
  - `lf task run` output names each handover it wrote and the person.
- **Desktop.**
  - Every Task workspace header has a **Hand over** button. Its popover
    offers:
    - a picker of people (plus email entry) built from `lf person list`, and
      your own Machines from the local table;
    - a note field;
    - the holder-only rule.
  - The Available sidebar filter includes Tasks picked up here that have no
    live Session.
  - A Task held by someone else shows "With Dustin" and never counts as
    Available for you.
  - The Workflow graph marks each person node with its person.
- **Agents.** Workflow guidance (`Workflow::guidance`) names the node's
  addressee. At a node addressing someone else, it says "handed to Dustin;
  leave the branch alone" in place of edge commands.

## Source of truth and affected surfaces

- **Who holds a Task:** the ownership log, which is exchanged. The holder,
  due-handover state, pickup and pending decision are derived from it, this
  Machine's pickup setting and the local Workflow row.
- **Who people are:** Person records on `refs/loopflow/people/*`, written by
  that person's Machines, cached locally in a `people` table. Provider user
  lookups answer what the records lack.
- **Pickup:** the local `machines.pickup` column for this Machine. Never
  exchanged.
- `task_workflows` stays local. On the receiving side it is seeded from
  `handover.graph`, and it is never exchanged as its own record.
- **Schema:** one migration draft.
  - `task_ownership` gains `handover TEXT` (JSON `Handover`, NULL otherwise).
  - `task_workflow_moves.kind` allows `received`.
  - `people(key PRIMARY KEY, name, accounts JSON, updated_at, fetched_at)`.
  - `machines` gains `pickup INTEGER NOT NULL DEFAULT 1`.
- **Wire DTOs:**
  - `TaskOwnershipEntry` adds `handover`.
  - The Task runtime owner gains `is_you: bool` and
    `handover: Option<TaskHandoverView {from, to, node, edge, note, head, at,
    opened_at}>`. `opened_at` is the recipient's claim time; no Machine.
  - The Workflow node DTO adds `person`.
  - A new `PersonView {key, name, accounts, is_you}` is emitted by
    `lf person list --json`.
  - Fixtures go under `tests/fixtures/dto/`, with Swift mirrors in
    `swift/Loopflow/Models/TaskWork.swift` and the owner model.
- **Docs:** update `docs/lf.md` "Work with another person" and its ownership
  table, `docs/architecture/machines.md` (cross-person handoff is no longer
  deferred; Machines stay local; pickup), workflow authoring (`person:`), and
  regenerate `docs/lf-reference.md`.

## Alternatives considered

- **Machine records on `refs/loopflow/machines/*`, Person derived from them.**
  The previous revision of this design. Superseded by Jack (2026-10-21): only
  the Person reaches Git. It also published every Machine label to every
  teammate and needed read-time pickup resolution across records with
  last-writer-wins on a `pickup` flag. Local pickup has no conflict to
  resolve.
- **Exchange Workflow position as a planning record.** Rejected. It doubles the
  authority over position and crosses "planning exchange excludes execution".
  Only handovers need the graph.
- **People in the planning ref.** Rejected. Under Linear the planning ref is
  not exchanged, and a roster has to work under every transport.
- **Provider users as the only roster.** Rejected (Jack): Git-only planning
  has no users. Providers answer first where they can; the record is the
  backup.
- **Pickup off by default (opt in per Machine).** Rejected for now: a
  single-Machine person would silently receive nothing. Listing a Task on
  several of your Machines is harmless, since the first open claims it.
- **Keep the Linear assignee one-way.** Rejected by Jack (question 7): a
  reassignment in Linear transfers the Task. The log stays authoritative
  because observation writes the reassignment into it.
- **Reviewer runs the next edge on their own Machine.** That is a takeover. It
  stays available as a manual handover.
- **Auto-run the handed-back edge on import.** This breaks "imports create no
  requests". Kept as an open question.
- **GitHub review requests.** Dropped by Jack on 2026-10-21. Notification is
  provider assignment, or nothing.

## Wild failure, pre-empted

- **A handover silently strands work.** Due-for-handover is derived, so a
  failed push shows a button rather than losing the Task.
- **A handover lands only on a Machine the recipient never uses.** Pickup
  defaults on everywhere, and only an explicit `pickup off` hides it.
- **Nobody opens it.** The sender sees "not opened yet" with its age, can
  re-address with `lf task handover`, or take it back with `lf task claim`.
- **Two Machines of one person both publish their Person record.** Lease plus
  merge by provider; the record only changes when an account or name does.
- **Two branches or two PRs.** Depends on LOO-555 item 2: every holder reuses
  the recorded branch.
- **Dustin's operator claims the returned Task.** Depends on LOO-553: operators
  leave others' Tasks alone.
- **An email typo in `person:`.** Same as nobody opening it.

## Exclusions

- Approval semantics beyond choosing an edge.
- Multiple people per node, and Teams.
- Session history transfer.
- GitHub notification.
- Short names in workflow files.
- Notion.
- Mobile.

## Dependencies

- **LOO-555** must land first with the email person key, branch reuse and Git
  comment steers. 878ecf no longer needs its Machine ref.
  - Jack Heart (2026-10-21) resolved cron visibility: crons are private by
    default and published one at a time with `lf cron publish`, each carrying
    its person key, Machine key and Machine label. LOO-555 was steered to
    drop the general Machine record (`ops/machine_record.rs`) in favor of
    published crons only.
  - `task_ownership.machine_label` stays, filled only from a public
    Machine's public name (question 11). LOO-555 was steered accordingly.
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
- If LOO-555 lands them: a general Machine record or
  `refs/loopflow/machines/*` beyond published crons, and any
  `machine_label` written from a private Machine, with their tests.

Forbidden:
- A separate transfer table or record type alongside the ownership log.
- Any published Machine record or Machine list. A Machine's private label
  never leaves it; its public name appears only when it is public.
- Pickup stored anywhere but this Machine's local row.
- Exchanging `task_workflows`.
- Launching from import.
- A second "available" or pickup judgment in Swift. Rust emits `is_you`,
  `handover` and pickup, and Swift only filters.

## Slices (one PR, after LOO-555)

1. Node `person` in the definition, the graph DTO and validation.
   Focused test: `cargo test -p loopflow --lib workflow::`.
2. Person record publish/fetch, the `people` cache, `lf person list`,
   `machines.pickup` and `lf machine pickup`.
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
  world with Homes `jack`, `dustin` and `dustin-mini` (pickup off), the demo
  script holds end to end.
  - Dustin's `task list --available --json` contains w-1 with `node: design`
    and Jack's `head`; `dustin-mini`'s does not.
  - Dustin's checkout HEAD equals `head`.
  - After the handback, Jack's `task status --json` owner `is_you`, with the
    handover `edge: ship`, and no field anywhere in Jack's JSON carries a
    Dustin Machine label.
  - Jack's bare `task run` starts `ship`.
  - A failed push leaves Jack's Task due for handover, with no handoff entry.
  - `lf task handover --to dustin-mini` from Jack's Home fails, listing Jack's
    Machines and suggesting `--to dustin@example.com`.
  - `git ls-remote origin 'refs/loopflow/*'` lists `people/jack@example.com`
    and `people/dustin@example.com` and nothing under `machines/`.
- `cargo test -p loopflow --lib task_ownership`: under a Linear test double, a
  handoff queues the recipient's Linear account as assignee, and a handback
  queues the owner's. A reassignment observed in Linear appends exactly one
  person-level claim for the new person across two observing Machines, and the
  previous holder's running Flow keeps running with a steer.
- `cargo test -p loopflow --lib person`: two Machines of one person publishing
  different provider accounts converge on one record holding both.
- `swift test --package-path swift --filter WorkModelTests`: a Task picked up
  here with no Session appears under Available, and a Task held by someone else
  does not.
- `cargo test -p loopflow --test dto_fixtures` and the Swift fixture tests
  round-trip the new fields and `PersonView`.
- Review and demo: Jack and Dustin run the demo on the lf-world with Desktop
  open on both Homes.

Check: kickoff revision only; no build or tests run (prose).
