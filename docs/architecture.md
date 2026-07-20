---
layout: default
title: Architecture
---

# Architecture

Loopflow separates durable Work authority from provider conversations.

```text
Wave / Project / Task Work
└── Epoch
    └── Run                         scheduler claim and containment
        ├── AgentInvocation         one provider conversation
        │   └── Turn
        │       └── Ask ── Answer
        └── AgentInvocation         another conversation under the same supervisor
```

A Wave is a durable operating context. A Project is one measured bet inside a
Wave. A Task is concrete work inside a Project. Project trees do not recurse,
and repository writes belong to Task worktrees.

## Execution ownership

One non-ended Run owns an Epoch's execution authority and physical
containment. Its opaque `LF_RUN_LEASE` is the only capability that permits a
process to write as that Work.

A Reserved Run has no containment. Active and Stopping Runs have one complete
tmux or process-group identity. An Ended Run retains any containment it
acquired. Interrupt and recovery target that Run containment directly.

An AgentInvocation records one concrete provider conversation: provider,
model, account, surface, resume token, timestamps, and Turns. Its optional
`supervising_run_id` is provenance, not authority. Starting an invocation does
not rotate the Run lease, reserve another scheduler slot, or change the
interrupt target.

If a provider conversation fails while its runner remains live, the Run may
start another AgentInvocation. If the runner containment is lost, recovery
ends the observed Run and reserves a new Run with `retry_of` pointing to it.

## Durable input

Loopflow keeps unsolicited direction separate from targeted questions.

### Steer

Steer changes a Work's authored input. It advances the Epoch Basis and remains
durable until a successful later boundary proves incorporation. Live provider
delivery is a latency optimization, not the semantic receipt.

```bash
lf task steer INF-123 "keep the public name"
lf work steer task task_... "show the failing fixture"
```

### Ask and Answer

Ask/Answer is Turn-local tool I/O. It does not move Work Basis, enter the Steer
queue, or advance a flow step.

```bash
lf ask "Which behavior should this proof cover?"  # blocks in the child Turn
lf ask wait                                       # recover after shell loss

lf work asks                                      # User or ambient parent inbox
lf work answer ask_... "Cover stale authority."   # first authorized answer wins
```

Each exchange stores only:

```text
Ask(id, turn_id, route, question, asked_at, nullable Answer fields)
```

Work and Basis are derived through:

```text
Ask -> Turn -> AgentInvocation -> Run -> Epoch -> Work
```

The schema therefore cannot persist a question that claims one Work while
pointing at another Work's Turn. A partial unique index permits at most one
unanswered Ask per Turn while allowing sequential follow-up questions. A check
constraint permits either no Answer fields or one complete Answer. The answer
update writes only while `answered_at IS NULL`, so concurrent writers cannot
overwrite evidence.

The route is derived when the Ask opens:

- a child routes to its immediate parent Work;
- a supported interactive root may route to the authenticated User;
- a headless root without either route fails instead of waiting forever.

A Parent Ask accepts only the active Run lease for that exact parent Work. A
User Ask accepts only User authority. Siblings, children, unrelated Runs, and
stale parent leases fail closed.

An unanswered Ask is actionable only while its Epoch is open and its Turn has
not completed or been intentionally interrupted. Abandoning Work or
interrupting the Turn makes the exchange historical without inventing an empty
Answer. Provider, shell, or runner loss leaves it recoverable.

`lf ask` commits before it wakes the parent, polls without consuming model
tokens, retries the wake, and prints the recorded Answer to stdout. The
provider sees an ordinary long-running shell command; Loopflow needs no
provider-specific injected tool or mid-turn message transport.

Each Task Ask and Answer also enqueues a Linear issue comment in the same
transaction. Linear publishes afterward: failures remain in the durable outbox
for retry and cannot roll back the exchange or delay `lf ask` after the Answer
commits. A stable marker lets recovery adopt a remotely-created comment instead
of posting it twice.

## Flow execution

Task flows run serially. A Turn blocked inside `lf ask` remains the current
Turn, and the Task remains Running because its Run and containment are still
live. The playhead advances only when the enclosing Turn completes.

Project and Wave core conversations no longer receive child questions as
Steers or special control turns. The current durable-Ask slice exposes explicit
`lf work asks` and `lf work answer` servicing. Detached Project and Wave answer
agents are a separate follow-up: they will use fresh AgentInvocations under the
parent Run without receiving its lease or disturbing its core conversation.

Interactive Demo handback is also separate. Invocation handback remains only
for the existing opaque interactive surface until Demo owns that protocol.

## Runtime topology

```text
App / CLI -> shared local SQLite, Linear, GitHub

lf start (machine-local) -> Home resident -> Wave listener -> Wave resident
parent or CLI -> reserve Run -> __work project -> Project runner -> Task Runs
parent or CLI -> reserve Run -> __work task -> Task runner -> worktree -> PRs
```

Each Wave records one Home placement. Optional `owner` and `home` fields in its
goal control automatic startup; they are policy, not authorization. Ordinary
commands run on the current machine: named `lf start <wave>` records the local
Home and starts it, while bare `lf start` starts only policy-matching, locally
placed Waves. Crossing machines is explicit through `lf ssh <HomeId> ...`, and
the target proves its stable Home identity before acting. Hostnames and SSH
routes may change without moving the Work. `lf wave` keeps a foreground
development path.

At startup, `lfd` ensures the machine-local Home resident. The resident starts
every eligible Wave known to the local store across repositories and owns the
per-Wave listener children; `lfd` is not a remote control API.

Current truth is split deliberately:

| Source | Owns |
| --- | --- |
| SQLite | Work, Epoch, Run, invocation trace, Turn, Steer, Ask/Answer, placement |
| Repository | Wave goals and `MEMORY.md`; Task worktrees and authored changes |
| Linear | shared Wave/Project/Task planning truth |
| GitHub | branches, pull requests, checks, and merges |
| SSH | explicit foreground reach to another Home |
| Wave journal | Wave conversation and resident loop events |

## Public projections

`lf status` and `lf roadmap` derive lifecycle from Epoch, Run, and Wait. Pending
User questions are derived from answerable Ask exchanges rather than stored
attention flags. `lf trace --json` includes Invocations, Turns, Asks, and their
Answers. Invocation attach DTOs carry attach and handback data only; provider
conversation rows no longer carry Work attention or reviewer policy.

DTO fields are required unless their type is explicitly optional. Rust and
Swift mirrors do not hide drift behind defaults.

## Invariants

- One non-ended Run exists per Epoch.
- Only the current opaque Run lease writes as Work.
- Run containment, not invocation ordering, is the interrupt and recovery target.
- Every Turn belongs to one AgentInvocation; every Ask belongs to one Turn.
- One Turn has at most one unanswered Ask.
- An Answer is complete, authorized, immutable, and first-writer-wins.
- Ask/Answer never allocates an Epoch revision or enters Steer delivery.
- Interrupted Turns and terminal Epochs expose no actionable Ask attention.
- No Session, control Launch, Feedback, Continue, reviewer flag, or invocation
  attention column participates in the current runtime.
