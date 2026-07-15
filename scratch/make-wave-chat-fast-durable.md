# Durable Wave Chat

## User-visible outcome

Wave Chat is the dependable default place to talk to a Wave. Selecting a Wave
paints its latest saved conversation before process or network refresh. A
stopped or recovering Wave keeps a readable thread; missing or damaged history
is named honestly. A message accepted by the listener appears as a durable user
turn without waiting for trace capture or an agent body. Repeated equivalent
operational failures occupy one actionable row, and references in authored
prose are linked in place instead of repeated as detached context chips.

## End-to-end proof

Given a Wave journal with more than 12 turns, an incompatible tail, three
equivalent failed attempts, an absent resident body, and authored prose naming
`W2-174`, `Project Mac Surface UX`, `PR #934`, `commit ae375a502`, and
`scratch/qa-findings.md`:

1. `lf chat --history --json --limit 12 -w <wave>` returns the latest readable
   turns from the journal, marks the snapshot partial and truncated, and does
   not require a listener or mutate the incompatible tail.
2. `WaveChatConnection.start()` installs that snapshot before endpoint
   discovery, then reconciles listener replay by stable turn id without erasing
   the partial-history warning.
3. The transcript shows one failure notice with count, latest age, recovery
   state, and disclosed raw attempts; authored failure prose remains in
   chronology.
4. Twenty message posts made while no agent body or trace-capture work is
   available each return the journaled user turn. The composer remains usable
   while acknowledgement is pending, and a failed post restores the unsent
   draft rather than inventing a delivered turn.
5. The authored references themselves are interactive. Task and Project
   selection opens the existing work inspector, a published PR opens its known
   URL, and explicit commit or repo-relative file evidence opens its resolved
   target. Hover discloses a compact preview from local evidence. No separate
   reference chip repeats the prose.

Rust journal/server tests, the shared DTO fixture, Swift connection and
reference-parser tests, and Mac text-view interaction tests cover the path. The
real Product Wave remains readable after its existing capture failures.

## Source of truth

- `.lf/journal/waves/<wave>/journal.jsonl` under the Wave's origin repository is
  the only persisted conversation record. History JSON is a bounded read-only
  fold; SSE is the live continuation of the same fold. Swift writes no second
  transcript.
- The existing `lf status --json` `WaveWorkMap` is the local metadata source for
  Task, Project, and PR references. The Wave detail parent owns one asynchronous
  work-map read and shares it with plan and Chat; reference enrichment never
  gates transcript paint.
- Git in the Wave origin repository is the evidence source for explicit commit
  hashes and repo-relative file paths. Resolution is local and asynchronous;
  Chat never waits on GitHub or trace capture.
- A successful `POST /messages` response contains the user turn after the
  runtime has appended it. That returned turn is the durable acknowledgement;
  no second delivery ledger is introduced.

## Typed reference contract

Detection is conservative and presentation-only; journal text and wire DTOs
remain unchanged.

- Task: an exact identifier such as `W2-174`, resolved only when the current
  work map contains that Task.
- Project: the explicit phrase `Project <slug or exact title>`, resolved only
  against the current Wave's work map. Ordinary title-like prose is not linked.
- PR: `PR #<number>`, resolved only when a Task PR publication in the work map
  carries that number and URL.
- Evidence: `commit <7-40 hexadecimal hash>` or a backticked repo-relative
  `path[:line]`. The target must resolve inside the Wave origin repository;
  absolute paths, parent traversal, missing files, and unknown commits remain
  plain text.

The pure detector returns ordered prose/reference runs and preview metadata.
`SelectableAssistantMessageTextView` renders attributed link ranges, preserves
selection, intercepts internal navigation, and owns hover preview. User and
assistant authored prose use the same renderer. Task and Project activation
reuse `WaveWorkSelection`; PR activation uses its stored URL. Evidence opens the
local file/line or the repository's commit URL when a GitHub origin can be
resolved, with the popover remaining useful when no external URL exists.

## Affected surfaces and consumers

- Rust journal read path and `lf chat --history --json`: complete, missing,
  partial, and unavailable snapshots without listener dependency.
- Shared history DTO fixture mirrored by Rust and Swift.
- `WaveChatConnection`: local history before network refresh, stable-id replay,
  and durable POST acknowledgement.
- `WaveDetailPane` / `WavePlanView`: lift the existing work-map read to their
  parent so plan and Chat consume one snapshot without coupling Chat paint to
  its completion.
- `MessageRow` and `SelectableAssistantMessageTextView`: inline typed references,
  selection, activation, and compact hover preview for both conversation roles.
- Failure presentation: one exact-equivalent operational roll-up while the raw
  journal and disclosed attempts remain intact.
- CLI and Mac docs: history behavior, acknowledgement semantics, and reference
  fallback.

## Absent and error states

- No journal: `missing`, not an empty healthy transcript. The UI says no saved
  conversation exists and can still connect or start the Wave.
- Valid empty journal: `available` with zero turns.
- Malformed or incompatible line: `partial`; the readable prefix renders and
  later live connection does not claim the durable record was repaired.
- Other read failure: `unavailable`; live turns may still arrive, but the
  durable-history warning remains until a later local read succeeds.
- Listener absent: saved turns remain visible and composition is unavailable
  because this Task does not add an offline queue.
- Listener recovering with no resident body: composition stays enabled; an
  accepted post becomes a durable user turn even if no assistant can answer yet.
- Post failure: no delivered turn is invented; the unsent text is restored when
  doing so cannot overwrite newer composition, and the reason is visible.
- Work-map or Git resolution unavailable: transcript paint is unchanged and all
  unresolved references remain ordinary selectable prose. Never show a dead
  link, an empty popover, or a guessed target.

## Operational boundary

- Default history is the listener's 12-turn replay limit. Read the journal once
  and return only the requested tail.
- Install history before endpoint, work-map, Git, plan, run-history, trace, or
  network work. The measured warm process-level history path must remain below
  250 ms across 20 Product reads; PR #934 measured about 154 ms per invocation.
- A message acknowledgement waits only for loopback HTTP and the journal append.
  It never waits for trace capture, agent startup, body recovery, or an
  assistant response. Exercise 20 posts against a listener with no live body.
- Resolve one work-map snapshot per Wave-detail refresh. Reference parsing is
  linear in visible prose and performs no subprocess or network work on the
  render path; Git enrichment runs asynchronously and is cached per pane.
- Preserve append-only ownership: readers never create, truncate, repair, or
  lock the writer's journal.

## Ordered serial PRs

1. PR #934, open: bounded local-first history, honest evidence states, stable-id
   live reconciliation, failure roll-ups, raw-detail disclosure, fixtures,
   documentation, and measured Product-journal proof. Keep this slice coherent.
2. After PR #934 lands and Loopflow rotates the Task branch: shared work-map
   ownership, the typed-reference detector and inline AppKit renderer, compact
   previews/navigation, explicit pending/accepted/failed send presentation, and
   the 20-send recovery proof. This is the next pursue target.

## Exclusions

- Offline message queueing or another transcript/delivery store.
- Run History, Active Sessions, or broader Wave-detail redesign.
- Raw operational diagnostics in ordinary Chat; the journal remains available
  to later Control and audit surfaces.
- Linking Sessions, flows, or Waves; this Task's remaining typed contract is
  Task, Project, PR, commit, and file evidence.
- Changing journal format or removing raw evidence from audit storage.
