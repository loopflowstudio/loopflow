# Current cut: finish the attention handshake and delete Session control

[`docs/architecture.md`](../docs/architecture.md) is the canonical target.
This file names the next executable cut for `lf code`.

## Starting truth

The last two passes completed the data shape and exact capability half of the
large control cut:

- `InteractionReview`, `InteractiveHandoff`, their ids, dispositions, stores,
  commands, DTOs, and Swift models are deleted;
- `ChildCommand` and its source/state/effect/claim ledger are deleted;
- Review is derived from interactive flow position, a live Launch, and
  `attention: User | Parent(WorkRef)`;
- User and active parent Run use the same durable `steer` mutation, and
  `close_review` is Basis- and flow-position-fenced;
- `agent_launches` now carries product Run, Home, account, continuation,
  containment, opaque-boundary, attention, and handback facts;
- Work status, Wait, Run reserve/advance/stop, direct interrupt/abandon, typed
  CI claim, and active-Run operational fences exist;
- one opaque `LF_RUN_LEASE` now resolves directly to the exact active Run and
  legacy Session generation no longer reconstructs product authority;
- observed root assistant output is durable on Turn and Project drains oldest
  child attention before background work with live or interrupt-and-seed
  delivery;
- the trace-only `LF_RUN_ID` collision is renamed `LF_TRACE_ID`;
- the committed branch is 121,818 physical / 119,126 normalized Rust source
  lines, one below the adjusted target. The live-delivery playhead correction
  adds ten lines pending the next Session deletion.

Do not recreate Review, Handoff, ChildCommand, or a generic inbox. The line
target is achieved; behavioral proof and one authority path now decide done.

## What the implementation review exposed

1. Review route and pending attention are still one Launch flag. After a parent
   Steer, the Review must remain open but the current attention must stop until
   the child replies. The current model instead redelivers the same child before
   it has produced another Turn.
2. A child reply changes the parent's required context but does not allocate a
   parent Epoch evidence revision. Parent completion can therefore race new
   child attention without becoming stale.
3. Live child delivery initially allowed the repurposed background Turn to
   complete its flow step. The review fixed this locally by closing that flow
   body as Interrupted; deterministic live/seed-only tests still need to pin it.
4. Project and Task still have separate reservation/status/revoke/reap runners.
   They mirror lifecycle into Run instead of executing through it.
5. Project drains child attention, but the Wave resident does not yet use the
   same projection.
6. Persisting every assistant delta makes partial output crash-visible but
   rewrites the growing Turn row repeatedly. Keep correctness first; batch only
   after the attention protocol is proven.
7. The old 2,767-line CI lifecycle suite was deleted with the command ledger.
   Small behavior tests must prove current incident claim/preemption/fresh-head
   settlement without restoring mock-heavy Session tests.
8. Six unpublished branch migrations still claim canonical ordinals. Main now
   authors dependency-ordered drafts. Consolidate the final cut into drafts;
   do not maintain four schema steps that only expose intermediate states.

## Account-lease lesson from main

Main's account SSH work resolves authority once, gives descendants one opaque
handle, prevents nested widening, and fails closed when the broker disappears.
Run authority should have the same semantics without copying its SSH broker:

- one `LF_RUN_LEASE` value is the complete local capability;
- the opaque secret hash locates exactly one active Run; callers do not submit
  a Run id, Work id, Session id, generation, or Author;
- the store returns the Run, Work, Epoch, and current Basis after validating the
  capability;
- a child Work Launch receives its own Run lease, while nested commands inside
  one Run inherit the same fixed lease;
- malformed, stale, stopped, or absent agent authority fails closed. A User
  context is constructed only by an authenticated external entrypoint; absence
  of an env variable never selects User.

`LF_RUN_ID` currently names an older trace/process concept. Rename that
diagnostic variable or remove it before using the plain Run name publicly.

## Change

### 1. Finish the Review attention handshake

1. Keep the Review route (`User | Parent`) for the whole interactive flow, but
   treat `attention_at` as the current unanswered turn, not Review existence.
2. Derive Review from interactive flow + live Launch + route even when
   `attention_at` is absent. Query parent/User attention only when it is present.
3. A successful parent Steer clears `attention_at` after the Steer commits. It
   does not clear route, advance flow, or close Review.
4. A later terminal child Turn re-arms `attention_at` once. In the same
   transaction, allocate one `evidence` revision on the routed parent's current
   Epoch using the child Turn as the idempotent source. This makes parent
   completion stale and fixes the next boundary Basis.
5. `close_review` clears route and pending attention and advances the interactive
   flow under its existing Basis/position fence.
6. Opaque Launches re-arm through their explicit boundary/handback signal; do
   not invent Turns.

### 2. Complete parent scheduling

1. Project the oldest pending child Review for a parent from child Launch attention,
   current flow position, Basis, latest root Turn output, and stable child facts.
   Do not copy a prompt or create an inbox row.
2. Wave and Project check the same ordered control projection before every
   background boundary: direct User input, oldest child Review, then other
   actionable child evidence.
3. The already-selected parent Run and provider route conduct the control Turn.
   Live-deliver when the exact active Turn accepts it. Otherwise interrupt the
   background boundary and seed the durable control projection next.
4. A live-delivered control Turn closes its active background flow body as
   Interrupted; provider success cannot advance that playhead.
5. Preserve the background playhead separately from control input. After the
   parent steers or closes the child Review, resume at the next unfinished
   background step; never replay a completed step.
6. Parent control is read-only with respect to canonical main. Dirty main may
   prevent mutation work, not Review steer/close.

### 3. Delete the Session/body controller

1. Replace Project/Task reserve, activate, status, revoke, reap, settle, and
   successor execution paths with shared `reserve | advance | stop`.
2. Match `WorkRef` only where Project and Task domain policy differs: flow
   selection, closure, workspace/PR/CI evidence, and external bindings.
3. Move keeper recovery through the same Run transitions. Only positive
   `Absent` containment releases the active slot; `Present` and `Unprovable`
   remain fenced.
4. Remove Session/body process fields, statuses, generations, leases, env vars,
   DTOs, snapshots, and action matrices once their distinct domain facts have
   moved.
5. Keep shipped migration history, but leave one live implementation and one
   final dependency-ordered draft cutover. Do not preserve dual reads.

### 4. Restore small behavioral proofs

- an actionable current incident is claimed by the exact active Run and cannot
  reserve an overlapping repair Run;
- stale and land-time-only incidents do not preempt;
- a parked Review is preempted at most once without authoring a Steer;
- settlement records the first fresh repaired head, never the cached failed
  head;
- a parent Steer clears only pending attention; Review remains open and the
  child's next terminal Turn re-arms it exactly once;
- a stale/stopped Run lease cannot steer or close child Work;
- live and seed-only parent harnesses both service child attention before
  background work and resume the saved playhead.

## Delete

Delete the remaining execution vocabulary, not only its files:

```text
ProjectSessionStatus
TaskSessionStatus
ChildWriteLease
ChildLeaseState
project_sessions runtime/process columns and writers
task_sessions runtime/process columns and writers
Project/Task body generation, reservation, revocation, reaping, settlement
LF_PROJECT_SESSION_ID / LF_PROJECT_GENERATION / LF_PROJECT_LEASE_TOKEN
LF_TASK_SESSION_ID / LF_TASK_GENERATION / LF_TASK_LEASE_TOKEN
run_lease_for_child
RunLeaseToken::from_child
ambient fallback from missing Run authority to User
duplicate Project/Task lifecycle runners
```

Provider-native session/thread ids may remain private Launch continuation data.
Project and Task domain records may remain, but they are stable Work—not
executor Sessions.

## Done when

- one opaque Run lease identifies and authorizes the exact active Run without a
  caller-supplied Work, Session, generation, or Author;
- malformed, absent, stopped, or stale in-Run credentials fail closed;
- no Session/body generation is required to locate Work or authorize mutation;
- the same Run transition suite covers Wave, Project, and Task;
- `Unprovable` containment never releases the active Run slot;
- Project and Wave never begin background work while child attention is queued;
- the parent's existing Run/provider route conducts the child conversation;
- live providers inject child attention; seed-only providers interrupt and seed
  it without replaying completed background steps;
- the parent seed contains the child's durable root output and current evidence,
  so critique, questions, and brainstorming work without a Review record;
- every child reply allocates one parent evidence revision, so a racing parent
  completion or old boundary loses;
- live delivery and interrupt fallback both preserve the background playhead,
  and one unanswered child turn is not delivered twice;
- dirty canonical main cannot block Review steer or close;
- current actionable CI preemption and fresh-head settlement have focused tests;
- promotion and reteam fence on active Run plus containment only;
- `rg 'InteractionReview|InteractiveHandoff|ChildCommand|AwaitingHuman|Author::Human'`
  has zero production references;
- `rg 'ProjectSessionStatus|TaskSessionStatus|ChildWriteLease'` has zero
  production controller references;
- unpublished schema changes follow the draft-migration contract and expose no
  supported intermediate architecture;
- copied-database migration, full Rust tests, Swift tests, migration tests, fmt,
  and clippy pass together;
- Rust source stays at or below 121,819 physical lines on current main / 119,127
  normalized. Do not meet this by deleting behavioral proof.
