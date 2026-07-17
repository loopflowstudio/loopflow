# Current cut: exact Run authority and responsive parent control

[`docs/architecture.md`](../docs/architecture.md) is the canonical target.
This file names the next executable cut for `lf code`.

## Starting truth

The last pass completed the first half of the large control cut:

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
- the branch is 121,613 physical Rust source lines after current main's account
  lease and promotion additions: 118,921 normalized, 12,206 below the 131,127
  architecture checkpoint and already 206 below the adjusted size target.

Do not recreate Review, Handoff, ChildCommand, or a generic inbox. The line
target is achieved; behavioral proof and one authority path now decide done.

## What the implementation review exposed

1. `ambient_run_lease` still reconstructs authority from Project/Task Session
   id, generation, and body token. An absent legacy env bundle falls through to
   User. This violates exact Run authority and makes Session deletion
   impossible.
2. Project and Task still have separate reservation/status/revoke/reap runners.
   They mirror lifecycle into Run instead of executing through it.
3. `child_attention(parent)` is stored and tested as a query, but no Wave or
   Project scheduler drains it. The starvation bug therefore remains.
4. Attention identifies the child but carries no conversation content. The
   parent cannot conduct a brainstorming Review from a signal alone. Persist
   the root assistant text as optional Turn output and project the latest child
   output plus current Work evidence into the parent's control seed. This is a
   Turn fact, not a new Message or Review aggregate.
5. The old 2,767-line CI lifecycle suite was deleted with the command ledger.
   Small behavior tests must prove current incident claim/preemption/fresh-head
   settlement without restoring mock-heavy Session tests.
6. Six unpublished branch migrations still claim canonical ordinals. Main now
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

### 1. Make Run the only execution credential

1. Add a private encoded Run lease handle and one env parser. Keep the raw token
   out of DTOs, logs, Debug, and durable rows; store only its hash.
2. Validate the handle directly against the partial-unique active Run slot.
   Delete `run_lease_for_child`, `RunLeaseToken::from_child`, and all authority
   reconstruction from `ChildWriteLease`.
3. Inject the exact Run lease at Launch construction. Clear any inherited lease
   before starting a distinct child Run, then inject that child's lease.
4. Split authenticated User entrypoints from in-Run agent entrypoints. Never
   turn a missing or invalid agent lease into User authority.
5. Route Work status/steer/close/interrupt/abandon, CI claim, completion, PM
   reteam, and promotion fences through Run/Work identity only.

### 2. Make child attention executable

1. Record optional root assistant output on the observed Turn when the harness
   sees it. Partial/failed/interrupted text remains evidence; opaque TUI
   Launches still have no invented Turns.
2. Project the oldest live child Review for a parent from child Launch attention,
   current flow position, Basis, latest root Turn output, and stable child facts.
   Do not copy a prompt or create an inbox row.
3. Wave and Project check the same ordered control projection before every
   background boundary: direct User input, oldest child Review, then other
   actionable child evidence.
4. The already-selected parent Run and provider route conduct the control Turn.
   Live-deliver when the exact active Turn accepts it. Otherwise interrupt the
   background boundary and seed the durable control projection next.
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
- User and parent Review messages do not clear attention; `close_review` does;
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
