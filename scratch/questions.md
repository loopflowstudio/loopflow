# 878ecf open questions (kickoff 2026-10-21)

1. **Person key.** Resolved by Jack on 2026-10-21: the lowercase Git
   `user.email`, landing in LOO-555 (see decisions below).
2. **Who runs the edge after a review node.** Assumption: the owner's Machine
   runs it, and the reviewer only chooses it. The alternative is that the
   reviewer runs it. That is a takeover, and remains available via
   `lf task handover`.
3. **Auto-run on return.** Assumption: no. An import never launches work. The
   owner's live Session gets a steer, and a bare `lf task run` uses the chosen
   edge. An opt-in such as a Workflow-level `on_return: run` is possible later.
4. **CLI listing.** The brief names `lf session list --available`, but a handed
   Task has no Session on the recipient's Machine. Assumption: Tasks are
   listed with `lf task list --available`, and Desktop's Available filter
   includes them.
5. **Short names in `person:`.** Assumption: email only for now. A known-name
   resolution exists for `lf task handover --to` only.
6. **Notifying.** Resolved by Jack on 2026-10-21: no GitHub review request.
   Notification is provider assignment (Linear now, Asana with LOO-520), or
   nothing.
7. **Linear assignee edited by hand.** DECIDED by Jack Heart (2026-10-21): a
   Linear reassignment gives the Task to the new person as a take (a
   person-level claim, not a handoff: a handoff means the holder's Machine
   actively relinquished). The previous holder is superseded, not stopped;
   they get the superseded steer and a ledger entry. (Was: The
   ownership log is authoritative, and the assignee mirrors the holder. The
   alternative reads someone reassigning in Linear's UI as a manual handover,
   which would make Linear handovers work without lf on the sender side, at
   the cost of a second authority over who holds the Task.)
8. **LOO-555 landing shape.** On 2026-10-21 LOO-555 was at `implement`, with a
   committed design still using the `planning_user` UUID and
   `refs/loopflow/crons/<id>`. Assumption: if it lands that way, 878ecf's
   first slice converts both in place, with no dual reading.
9. **Non-public Machines.** DISSOLVED by Jack Heart (2026-10-21): Machines
   stay private. There is no public flag; cross-person handovers address a
   person, and only your own Machines can be named. (Was: Assumption: `lf task handover --to <someone's
   private Machine>` is refused at the sender, with the public Machines
   listed. The refusal exists only because Jack asked for "public". The
   flexible alternative hands to that Machine's person and lets their pickup
   rule apply.)
10. **Person record.** RESOLVED by Jack (2026-10-21, later): a Person record
    in Git (`refs/loopflow/people/<email>`: key, name, provider accounts),
    written by any of that person's Machines with lease + merge. No Machine
    records.
11. **LOO-555 publishes Machines.** Its uncommitted `implement` tree adds
    `refs/loopflow/machines/<id>` (`ops/machine_record.rs`, for teammates'
    cron visibility) and `task_ownership.machine_label`. Both contradict
    "Machines are never published". Assumption: LOO-555 drops both before
    landing; otherwise 878ecf deletes them in its first slice. Open for Jack:
    where teammates' crons become visible, if anywhere (one option: on the
    Person record, with no Machine named). LOO-555 had not received this
    direction as of 2026-10-21.
12. **Pickup default.** Assumption: on. Every Machine of yours lists
    person-addressed Tasks until `lf machine pickup off`; the first open
    claims. Alternative: opt-in, which strands handovers for a person who
    never marks a Machine.
13. **Machine ids in ownership entries.** Assumption: entries keep today's
    opaque `machine_id` (your own Machines need it to agree which one holds
    the Task); teammates' views never resolve or show it.

## Jack Heart's direction (2026-10-21, design review)

- Drop the GitHub review request. The real question is how a handover knows
  about Dustin and Dustin's Machine.
- Addressing: either use the person's account on the planning provider
  (Linear, Asana, Notion), or put a Machine field on the handover and parse
  it; the Machine that matches picks it up.
- Machines can be public (or a person marks a Machine public). Also allow
  "just this user", with a Machine declaring "I am the default for this user;
  I pick it up" (e.g. the laptop).
- Without a provider, put a basic users system into Git. Survey user and team
  records on Asana, Notion and Linear; put their common core into Git and use
  it as the API to all three.

## Jack Heart's decisions (2026-10-21, after the people research)

1. Git serves as the backup where providers fall short: provider user
   accounts first; Loopflow keeps Person/Machine (and Team) records in Git
   for what providers cannot hold (Machines, default pickup, teams where
   missing).
2. Notion may be cancelled if it is what blocks the shared core (no email
   lookup, no teams). Design to the Linear+Asana core; do not bend for Notion.
3. 878ecf takes on LOO-555's identity work. Farmed out to LOO-555 to land
   first (steered 2026-10-21, while its autonomous Flow was running): person
   key = lowercase Git user.email; per-Machine ref generalized to
   `refs/loopflow/machines/<machine-id>` {machine_id, label, person_key,
   person, crons}. 878ecf adds public / default-pickup fields, provider
   accounts, teams, and the handover itself on top.
- Drop the GitHub review request entirely; notification is provider
  assignment, or nothing.

## Jack Heart's direction (2026-10-21, later): only the person reaches Git

"the only thing that reaches git is the person, and then you can mark machines
to pick up things that match 'you'". Machines are never published. Git holds a
Person record (key = lowercase email, name, provider accounts). Pickup is a
local setting on each Machine: "this Machine picks up work addressed to me".
No `refs/loopflow/machines/*`, no `lf machine list --all`. This supersedes the
Machine-record design and the farm-out of `refs/loopflow/machines/<id>` to
LOO-555 (open: what happens to LOO-555's cron visibility, which used that ref).
- Refinement (Jack Heart, same day): publicly visible Machines are an
  option, off by default. Person always reaches Git; a Machine record
  (`refs/loopflow/machines/<id>`, LOO-555, carries crons) is published only
  on opt-in. Assumption: visible is not addressable; handovers still name a
  person, pickup stays local. Both Tasks steered.
- Refinement (Jack Heart, same day): the opt-in is per cron, not per
  Machine (`lf cron publish`). No Machine records reach Git. LOO-555 and
  878ecf steered. A published cron carries its Machine key (machine_id).
