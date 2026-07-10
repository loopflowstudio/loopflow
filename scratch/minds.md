# Minds: inhabit, delegate, promote

## Problem

A wave that wants work done has exactly one move: dispatch. `wave_pursue.md`
says *"Launch project or task loops for clear planned work"* and allows
direct execs only for *"hot, now problems or trivial single-file changes."*
`LOOPFLOW.md` makes it doctrine: *"Inline edits are only for trivial fixes
smaller than the cost of dispatching."*

Two costs follow.

The manager never touches the work. Delegation is treated as a procedural
requirement rather than as what it is — a parallelization technique. A wave
with one project still spawns a child to pursue it, for no gain.

And the user acquires a conversation surface per level. Wave, project, task:
three places to look to do one thing. Steering a running child means either
posting into a mind that stopped listening at birth, or attaching to its tmux
session and typing — the only mutation path in the system that never touches
the journal.

The runtime does not require any of this. `flowloop/README.md` already says
*"There are no tiers in code: `task` and `project` are just flows we define."*
`spawn_wave_pass` and `run_pass` issue the same command. The three noun-flows
are isomorphic — `{task,project,wave}_{clarify,pursue,mutate}`. The tiering
lives entirely in six markdown files.

## Model

### A mind is a read set

Not a process, not a worktree, not a channel. `wave_context.rs`:

> every `lf` run born inside a wave inherits the wave's recent conversation and
> curated memory ... the unified context flowing back into every publisher **at
> birth**.

Spinning up a new mind freezes a read set at birth. Using your own mind means
the work runs in something that keeps re-reading the live thread.

This is a property of **residency**, not of delegation. A worker's read set
freezes because the process dies. A wave's stays live because a resident
persists and re-reads each pass. The two only appear to coincide today.

### Three verbs

| Verb | What it is | Cost |
|---|---|---|
| **Inhabit** | `lf loop <flow> "..."` and wait | serialization — the wave is one long tool call until the bit flips |
| **Delegate** | the same command, detached | parallelism's price — you find out at report boundaries |
| **Promote** | grant residency | a process, an endpoint, a cadence, a budget; the parent stops overhearing |

Inhabit and delegate are **the same command**, differing only in whether the
wave waits. Inhabiting is not a mode of the wave — the wave flow never changes,
`spawn_wave_pass` stays `"wave"` — it is the wave LLM choosing to run
`lf loop project ...` and block. There is still no fourth cell: "other mind, my
hands" does not exist.

Both forms fork a worktree and keep a private transcript. What distinguishes
them is attention: a blocking loop is where the wave's time goes, and where its
thread's ear goes (see the seam below). Every loop child re-reads the wave's
live memory and thread at each of its pass boundaries, so in read-set terms the
work always runs inside the wave's mind — foreground or background.

**Delegate** when the work neither needs your attention nor deserves it yet.
The seed test `project_pursue.md` already states: *"The seed is the task's
whole handoff — make it computable on its own."* Detaching is also context
hygiene — a refusal to let the work's detail occupy your attention. That is the
real content of the transcript-bloat argument, which turns out to be principled
rather than incidental.

**Promote** when you need to steer it while it runs — a second thread, a second
ear, on purpose.

### Waves are the only minds

`wave/channel.rs` already says this:

> Child channels have NO loop: they are pure streams (no `LoopState`,
> no memory — a work line's notes are files; **MEMORY.md is wave identity**).

There is no way to launch a mind without promoting to a wave. Task and project
loops are runnable *from within* a mind:

```bash
lf loop task "..."           # block on solve — the wave waits for the bit
lf loop task "..." --detach  # background solve — server-owned; concurrency, the only reason to
```

Entering a task or project loop inherently forks a worktree. That is not a
cost to weigh; it is what the verb means.

### Two wires: the bus and the thread

The `say` op is two different things fused. Today it is both agent-to-agent
signaling (wakes the loop, coalesces into the inbox) and the human chat
(folds into the thread the Mac renders). Every steering problem in this design
traces to that fusion. Split them.

**The bus** — pubsub, agent-to-agent only. Channels are topics:

- **Channel** — a topic. What the stream is about. Any `lf` exec may post to or
  listen on any channel it has the capability for.
- **Attribution** — a byline. Who said it. **Server-stamped from the token.**
- **Family head** — routing. Which server holds the pen, which journal file.
  Keep it; it is a fact about single-writer discipline, not about meaning.

**Writes go down. Reads go up.** *(Superseded by §8 — kept for the reasoning,
not the rule.)* The original sketch: publish permitted iff
`matches_prefix(target, writer_channel)`, with `writer_channel` derived
server-side from the token — no upward write anywhere, `lf chat --parent`
becomes sugar for `--channel <self>/report`. §8 keeps the *shape* (a hand
speaks in its own room; the parent chooses to listen) but drops the gate:
publish is open, and the security boundary is attribution — the server stamps
who spoke from the token, so a byline cannot be forged. Routing rules turned
out to be permission theater once bylines were honest; the default posture
(narrate on `<self>`, report up by publishing where the parent listens)
survives as convention taught in the loop skills, not as an enforced rule.

**Wake is subscription.** The loop wakes on reports, heartbeat, and cron.
It does not subscribe to its own narration — so a wave inhabiting a task cannot
wake itself. Not guarded against; structurally impossible once narration and
address are different things. A hand that finishes silently is a hand that
never reported, and that is legible as its failure.

Security consequence, not optional. `sender_attribution()` (`chat.rs:302`)
builds the byline from the caller's env, and the wire carries
`from: Option<Attribution>` (`server.rs:334`). Client-claimed. That is safe only
while the channel is ownership-derived — the address pinned the byline. Topics
unpin it: a leaked worker token would post as the wave. **The token names the
writer; the writer does not get to say.**

Cost, in `channel.rs`. Ownership naming inverts a channel to a worktree
(`child_worktree_path`), and *"its journal lives IN THAT WORKTREE… it travels
with the branch and dies with it."* A topic cannot live in a worktree, because
many processes post to it.

The resolution is not to move the journals — it is to delete them. A journal
buys exactly one thing over a broadcast bus: delivery to a subscriber who was
not listening at publish time. That is a need **minds** have (their loop queues
between passes; their thread replays on reconnect), never topics. So journaling
is a property of minds, not channels: publishers journal nothing; a subscriber
records what it needs to remember in its own journal. The wave already does
this — every child `say` lands in the parent's journal attributed and queued
(`runtime.rs` calls it the fold-upward doctrine, but it is really the parent's
subscription, recorded in the parent's own journal). Once that copy exists the
child journal has no reader: the Mac drops tagged frames, live subscribers ride
the broadcast, and at land the file is deleted unread. It is not even a
distributed record — the family head holds every pen, so the same process
writes the duplicate into a different directory.

One journal per served mind, zero per channel. `ChildChannel`, the
consumption-local machinery, the worktree-liveness dance, and the FLAGGED
archive fallback (`~/.lf/journal/<repo>/<worktree>`) all delete. A detached
loop's driver holds a live subscription for its own lifetime and queues in
memory — a queue that dies with its loop is honest, and the slow path (the hand
re-reads the wave's thread each pass boundary) exists regardless.

**The thread** — the human surface. Not a topic. One UI-level thread per wave:
journal-backed, durable, never resets. The human is not on the bus.

### The thread is the product; the session is the current body

There is no separate chat LLM. The human connects to **the running pass
itself** — the session initialized with `wave_clarify`/`wave_pursue`/
`wave_mutate`, given the skill's seed as its first message, and then live to
the user interactively. The persona changes out from under you as the flow
moves through its skills, and that is honest: you are talking to the wave *at
a phase*. The UI shows one thread while the underlying harness rotates —
per-skill sessions, per-pass processes, even per-skill vendors
(`default_agent:` is skill frontmatter).

The scheduler already holds the inbox open during a pass —
`run_pass`'s `select!` (`wave.rs:439`) is `biased` toward the inbox and beats
pass completion. Mid-pass there are two verbs today: `Interrupt` kills the
child; everything else queues for the boundary
(`messages_during_a_pass_coalesce_into_one_boundary_pass`). The change is one
arm: forward the message **into the child's live session** (streaming-input
harness mode) instead of queuing it. Interrupt and coalescing survive
unchanged — restart-the-loop is a steering lever that already exists and is
already journaled.

Responsiveness becomes tool-boundary granularity (seconds to a minute), not
pass-boundary (30min–4h). When no pass is running, a human message journals,
wakes a pass, and the human is attached from birth. One thread either way.

What #845 established survives fully: session lifetime equals pass lifetime, no
resident LLM, log as truth. The session is disposable; the thread is durable.

Three seams:

- **The journal, not the harness transcript, is the continuity.** Every body
  receives recent chat history at birth — `<lf:wave-chat-recent>` already rides
  every launch (`wave_context.rs`: 12 turns, newest survive), and under this
  design it stops being context flavoring and becomes the mechanism that makes
  session rotation invisible: every body wakes up mid-conversation, already
  caught up. Which forces a rule: every in-session exchange journals *as it
  happens*. User messages already do (the `say` op); the pass's replies ship at
  the boundary today (`ship_output`) and must journal incrementally instead —
  anything said mid-session that isn't journaled is invisible to the next body.
- **`pass_timeout` will hang up on the user.** The timeout arm in the same
  `select!` kills the child at 30 minutes flat, mid-sentence if the human is
  mid-conversation. Presence extends the lease, or interactive passes get
  different caps.
- **Chat spends the engine's context.** The conversation lands in the working
  pass's window. That was the only genuine advantage of a separate face agent,
  and it is the price of one head.

### The mind has a playhead

> "in this world the mind is always in the middle of some flow. Let's imagine
> it sort of like a playlist/queue. The default is just looping N steps. But
> you can also enqueue an arbitrary flow, or you could skip to the next step"

The resident is not idle between passes. It always has a current step and a
next step. The wave flow is its default playlist: after the last step, the
playhead wraps to the first. The scheduler does not pre-materialize an infinite
queue; it advances a cursor through that default cycle.

An explicitly enqueued flow temporarily supplies the next steps. It runs in the
same mind, against the same thread and memory, then the default playhead resumes
where it left off. This is inhabitation made visible: running another flow does
not create another conversation or resident. The invocation may still carry a
placed worktree; placement is where the body acts, not whether it is another
mind. Detaching that flow is the separate choice that creates a hand.

```text
now playing   wave / pursue
up next       review-design / clarify
              review-design / pursue
then resume   wave / mutate
```

Skip interrupts the current body, journals the step as skipped, and advances
the playhead. It does not restart the wave or discard the rest of the flow.
Human steering still goes into the current body; enqueue and skip are explicit
controls over what body the thread will inhabit next.

The journal is the queue's source of truth. At minimum it records flow
enqueued, step started, step completed, and step skipped, so a process restart
reconstructs both the durable thread and its current playhead. The product can
therefore say what the mind is doing without exposing a session as the unit of
conversation.

> "after the current inner most invocation."

Enqueue attaches to the innermost active flow invocation. The current step and
the rest of that invocation finish first; the enqueued flow then plays before
control returns to the caller. If `wave` invoked `review-design`, and
`review-design` enqueues `research`, the order is:

```text
wave / pursue
  review-design / clarify
  review-design / pursue
  review-design / mutate
  research / ...              # queued on review-design
wave / mutate                  # return to the caller
```

The queue is therefore local to a flow frame, not one flat list attached to
the wave. Each invocation has a cursor and a FIFO continuation queue. Finishing
the frame drains that queue before popping back to its parent. The root wave
invocation is the default frame; after its continuations drain, its cursor
wraps and begins the next cycle.

### Navigation is stack-shaped

Complex flows should not be flattened into a playlist UI. The chat needs only
the local horizon:

```text
wave › review-design › pursue       # where am I?
now       review-design / pursue
next      review-design / mutate
queued    research                         # before returning to wave
```

The breadcrumb is the invocation stack. `Now`, `next`, and the current frame's
queue are the steering surface. An expandable execution map can show the full
flow tree, completed branches, loops, skips, and return points for audit, but
that complexity stays out of the primary chat path.

This requires stable invocation identity in addition to flow and step names.
The same flow may appear more than once in the stack or queue, so provenance
must carry an invocation id and step path; names alone cannot reconstruct the
navigation.

### The thread shows its bodies

One thread is assembled from many session streams. The UI should preserve that
provenance instead of flattening the handoffs away or turning each session into
a conversation.

At every body change, the transcript gets a quiet inline boundary:

```text
── wave / pursue · now playing ──
assistant output…

── review-design / clarify · enqueued ──
assistant output…

── wave / mutate · resumed ──
```

The primary label is the product meaning — flow and step — not an opaque
session id. Expanding the boundary reveals the exact session, harness, model,
host, run, timing, and termination reason for audit. Each streamed assistant
turn carries that provenance from `TurnOpened` onward, so reconnect and journal
replay reconstruct the same grouping.

Enqueue, skip, completion, crash, and timeout appear as boundary events in the
same thread. A session is therefore visible as the body that produced a span of
the stream, but never becomes a tab, room, or alternate chat history.

### What a hand is

A hand has a voice and no room. Its transcript is private — *"Trust worker
summaries; do not reread worker transcripts."* Its posts are public. What forks
under delegation is neither memory nor voice; **it is the transcript**.

A hand's ear is subtler than it looks. A one-shot `--dispatch` exec is seeded
once and deaf after. A **loop** is many births: every pass is a fresh
process inheriting `LFD_WAVE_ID`, and context assembly resolves the *wave* from
it. So a loop re-reads the wave's live memory and thread at every pass
boundary. Its ear opens once per pass, not once per loop.

If that holds for `lf loop` children (**verify**), then a hand is steerable
through the wave's own thread at pass granularity, and both `lf sub` and the
tmux door lose their justification without anything being built.

**A hand can report; only a mind can converse.** But a hand *listens* — because
it lives inside a mind.

### What promotion is

Promotion grants **residency**. Everything on the wave's list of five — memory,
cadence, budget, chat, project selection — follows from persisting and
re-reading. It is mechanically checkable: does it have a `.wave-endpoint`?

Promotion forces delegation, but not because the read sets diverged. Because
the pen moved. Two minds writing one log is the actual problem.

### Memory inherits; chat is passed

Opposite disciplines, and a wave is the unit of both.

**Memory is lexical scope.** A subwave reads the parent's live memory and
writes only its own. Context assembly walks `parent_wave_id` — the chain exists
in the registry, `lf chat --parent` already walks it, and `wave_context.rs`
assembles `<lf:wave-memory>` from one wave. Nothing is copied, so nothing rots;
one home per fact, inherited downward.

**Chat is a mailbox.** It stops at wave boundaries. Crossing requires explicit
replication by the child into the parent's thread. `<lf:wave-chat-recent>` does
not walk the chain.

These two sections are assembled by the same function today. They must stop
being.

**The boundary is the mind, not the process.** A hand lives *inside* its wave's
mind: it reads that memory and that thread, live, every pass. A subwave lives
*outside*: it inherits memory down the scope chain but not the thread. So "chat
stops at wave boundaries" and "a hand hears its wave" are one rule, not two.

### The invariant

- Worker → wave: the transcript is private, the posts are public.
- Child wave → parent wave: the child publishes to `<self>/report`; the parent
  subscribes.
- Memory crosses freely downward, but only because `lf memory add` already made
  it an authored statement.

> **Nothing crosses a boundary unless someone wrote it down on purpose.**

Raw records stay home; authored statements travel. This is what lets
log-as-truth survive nesting: every log is complete for its own scope, and what
leaves it is what a mind decided to say.

Under writes-down/reads-up the invariant is total. There is no exception,
because there is no upward write to except — including `driver.rs:206`, which
was never a crossing.

## Consequences

**Project count does not touch chat count.** Projects were never minds. The wave
holds three bets in one head; you talk to the head. Delegated workers already
post into the wave's journal. Nothing needs to be built for three projects to
share one thread — it needs to *not be undone* by a surface that renders every
child channel as a conversation.

**The planning ontology is untouched.** `STYLE.md`'s three nouns keep their
distinct lifetimes. What collapses is the execution model. Declared structure
stays flat; runtime structure is a tree that exists for a lease and then
collapses. **Inhabitation is a lease, not an identity** — a wave can *be* a
project for a stretch; it cannot *become* one, or it would inherit a
termination bit and stop being durable.

**Promotion is self-limiting.** Before promotion the parent overhears
everything; after, it reads dispatches. You gain an ear on the child and lose
ambient awareness of its work. The cost is paid by the promoter, in the
currency they care about, at the moment they choose. If chat flowed upward
automatically, promotion would be free and the wave roster would grow without
bound.

**Direct control has nothing left to do.** `tmux attach` exists because the mind
on the other side went deaf at birth. Fix the read set and the door closes on
its own. It is also the only mutation path that bypasses the journal — an
unattributed write, after `cf4aa764` took an append lock on everything else.

## Status

Where this branch actually landed. The Model above is the durable theory and
stands; this ledger is what a fresh session needs before touching code.

**Built.** The parent checkpoint was full-suite green. The CLI steer slice is
covered by its chat-command tests and the live-body done-when test.

| Change | Where |
| --- | --- |
| §1 `lf loop` — inhabit, delegate, `--detach`; `--dispatch` deleted | `bin/lf.rs`, `flowloop/driver.rs`, `wave/server.rs` |
| Batch and steerable are separate entrypoints: `lf loop` vs `lf serve` | `lf/mod.rs`, `wave/mod.rs` |
| Exec door pins a detached loop to its own wave | `wave/server.rs:774` |
| §2 Memory walks the parent chain; chat stays local | `engine/wave_context.rs` |
| §3 `lf project promote` — flow, skill, parent link, PM move | `ops/project.rs` |
| The playhead: model, FIFO nested frames, skip, retry-on-failure | `wave/playhead.rs` |
| Playhead durability across reconnect and restart | `wave/journal.rs`, `wave/runtime.rs` |
| `lf chat --steer` reaches live Codex bodies; attributed reports stay queued | `lf/commands/chat.rs` |
| §4 tmux door read-only · §5 backlogs allowed · §7 doctrine | `engine/builtins/**` |
| §6 Mac surface — five panes, playhead header, body provenance | `swift/LoopflowMac/Views/**` |
| §8 Channels are wires: `ChildChannel`, child journals, `child_worktree_path`, `scan_child_channels`, the worktree-liveness dance, and the FLAGGED archive all deleted | `wave/channel.rs`, `wave/runtime.rs`, `wave/server.rs`, `wave/registry.rs` |
| §8 Byline server-stamped from the channel; a forged `from` never survives | `wave/runtime.rs` (`deliver_to_channel`) |
| §8 `lf radio` = the agent bus; `lf chat` = the human↔mind thread; one transport, two verbs | `lf/mod.rs`, `bin/lf.rs`, `lf/commands/chat.rs` |
| §8 Doctrine: LOOPFLOW.md teaches radio; the five skills escalate with `lf radio --parent` | `engine/builtins/**` |
| §8 A hand reads its wave's thread — the two-section work-line overlay collapses | `engine/wave_context.rs` (`gather_channel_chat`) |
| §9 The db IS the bus: `bus_messages` + `bus_cursors`, publish is an INSERT, sweep rides the publish (1h window) | `lfdb/migrations/059_bus.sql`, `lfdb/sqlite.rs`, `lfdb/mod.rs` |
| §9 `lf radio` is its own command with no server in the path; `--steer` is a parse error | `lf/commands/radio.rs`, `lf/mod.rs`, `bin/lf.rs` |
| §9 `lf sub` polls the bus by prefix; the SSE thread-follower moved out to back `lf wavechat` | `lf/commands/sub.rs`, `lf/commands/thread.rs` |
| §9 The listener demotes to a subscriber: durable cursor, exactly-once fold, visible cursor jump past the window | `wave/bus.rs`, `wave/mod.rs` |
| §9 Broker deleted: `family_tx`, `ChannelFrame`, `tagged_turn_json`, `deliver_to_channel`, `subscribe_channels`, `?channel=`/`?prefix=`, `/messages`'s `channel` | `wave/runtime.rs`, `wave/server.rs`, `wave/channel.rs`, `lf/commands/chat.rs` |
| §9 Doctrine: LOOPFLOW.md's radio block loses the "wave must be running" caveat; byline reads as testimony | `engine/builtins/LOOPFLOW.md`, `wave/README.md` |

**Not built.** Each is a real gap, not a rough edge.

| Gap | Consequence today |
| --- | --- |
| A detached loop's driver holds **no** live subscription (§8 assumed one) → **§9 dissolves this** | `lf radio --channel <hand>` broadcasts to whoever is tuned in and dies there; steering a hand means speaking on the wave's thread. Under §9 the hand's ear becomes a poll cursor on the store bus — cheap enough to build, or to skip deliberately. |
| The token names the wave, not the hand → **§9 dissolves this** | The byline is stamped from the **channel** because one token per boot means the server can't tell hands apart. §9 removes the server from the publish path entirely: bylines become client-submitted testimony recorded beside the channel's evidence, and per-hand tokens stop being needed. |
| Mid-turn steer for Claude and OpenCode → *roadmap* | Only Codex steers; the others queue to the next body. Vendor-gated; the product question lives in `wave/product/projects/wave-chat.md`. |
| Composite flow nodes (and/xor/or/loop) as first-class playhead frames | They run through the internal headless `__flow-step` fallback (`flowloop/wave.rs:311`). |
| PM `project:<slug>` label removal on promotion → *roadmap* | The provider has no remove-label op; residual labels are recorded, not cleared. |
| Project-loop caps | `lf loop project` still inherits the generic 8-pass / 2-hour defaults (`flowloop/driver.rs:44`). |
| Foreground/background label in Active sessions | The run ledger doesn't persist the owner, so the Mac shows pass/worktree/liveness and declines to guess. |
| One writer per wave worktree | Nothing enforces it. This branch was written by two agents at once and it left a self-contradicting test in the source. |

**Fixed after the fact** — defects this branch shipped and a later session repaired:

- **Promotion could never start residency, twice over.** It spawned
  `lf wave <child>`, a subcommand this same branch deleted. Renaming it to
  `lf loop <child>` was not enough: `lf loop <name>` chose between *booting a
  listener* and *being a resident body* by reading `WAVE_SERVER_ENDPOINT` and
  `RESIDENT_TOKEN` from the environment, and tmux hands a promoting pass's
  environment straight to its child (verified empirically). The child wave's
  resident would attach to its **parent's** listener with the parent's token —
  a split brain surfacing as a 10-second timeout naming the wrong cause.

  The verb is now split by what it does, not by what it inherited:

  | Command | What it is |
  | --- | --- |
  | `lf serve <wave>` | boot a mind: listener, thread, playhead. Steerable. |
  | `lf loop <flow> <seed>` | run a bounded child loop to its bit. Batch. |
  | `lf __resident <wave>` | hidden; the body a listener spawns for itself. |

  `seed` is now required on `lf loop`, which deleted an exec-door case by
  construction (a seedless detached loop can no longer be spelled). Environment
  configures a process; it no longer decides what the process is.
- Three wave tests asserted pre-rename wire shapes (`"thinking\nmore"` for what
  is now exact stream concatenation; `{"id":null,"op":"interrupt"}` for what is
  now `{"kind":"interrupt"}`).
- Two `bin/lf.rs` tests still named the deleted `wave` command. They had never
  run: a failing lib target makes `cargo test` skip every later target, so the
  lib failures masked them. **Run `cargo test` to completion before trusting a
  green-looking suite.**
- Listener force-finalization closed the chat turn but left its playhead body
  active, so the respawned resident could not retry the same logical step. The
  janitor now closes and journals both records atomically.
- The Mac Open PRs pane filtered active runs, hiding the completed runs that
  normally own open PRs. `lf status` now exposes live PR state and title, and
  the pane filters all recent runs by open or draft PR state.

**Reduction pass** (post-review, triaged; details in Next implementation
session item 2). Applied in-tree and verified green: the `TurnFinished` +
`BodyFinished` terminal pair collapsed to one helper (was hand-copied at four
sites — the same drift disease this branch already caught once in its tests);
`bin/lf.rs` reuses a now-`pub` `LoopRun` instead of reimplementing it (the
copy existed because `pub(crate)` doesn't cross from lib to bin); the
`playhead.rs` error hint prescribing the unparseable `lf loop {wave}` now says
`lf serve`. Open: shared inbox arms + lease-renewal lift, `begin_interrupt`,
resolver consolidation, `require_loop_flow` inline. Kept deliberately:
`heartbeat_idle`.

## Next implementation session

Work that is schedulable now, ordered by what unblocks the most. Everything here
is ours to write; nothing waits on anyone else.

1. **A hand's ear.** §9 left this fork open and it stays open: the detached
   driver holds no subscription, so a `lf radio --channel goals.<run>` steer
   still reaches only whoever is tuned in right now. The wave's thread remains
   the ear a hand reliably has (it re-reads at every pass boundary). The bus
   makes the poll cheap — `read_bus_after` from a cursor in the driver's pass
   boundary — but nothing is built.
2. **Reduction leftovers from the review pass** (findings already triaged;
   1, 2, and 5's stale-hint bug are applied in the tree): factor the shared
   inbox-interrupt arms and lift the lease-renewal block (finding 3 — take
   the lease lift; judge the arms enum on readability, interrupt semantics
   should be lockstep across both loops), merge `interrupt_child`/
   `interrupt_harness` behind one `begin_interrupt` (4), finish the endpoint
   resolver consolidation (5), inline `require_loop_flow` (6).
   `heartbeat_idle` stays — deliberately: it is a real scheduler input with a
   plausible product future, and deleting it to satisfy a lint instinct would
   be reshaping production code around tests in reverse.
3. **Project-loop caps.** Needs dogfood data, not a guessed weeks-scale timeout.
   Run one real project loop first, then pick.
4. **Composite playhead frames.** Promote branch/loop internals out of the
   `__flow-step` fallback. Separate graph-runtime work; do it when the Mac's
   breadcrumb starts lying about nested flows.
5. **One writer per worktree — as a store lease.** §9 names the mechanism:
   one-brain-per-wave is already a pid-probed store row, and a worktree lease
   is the same row one table over. Four writers hit this worktree in one day.
   Until the lease exists, check for a live agent before working a wave
   worktree.

### Punted to the roadmap

These are product questions or vendor-gated capabilities, not tasks. They
outlive this branch, and `lf pr land` deletes `scratch/*` — so they live in the
wave, not here.

- **Mid-turn steer beyond Codex** → `wave/product/projects/wave-chat.md`, Open
  questions. Claude and OpenCode expose no true mid-turn steer, so "steer"
  either degrades honestly to a queue or means interrupt-and-refold. That is a
  decision about the send/steer/interrupt KR, and it cannot be scheduled by
  deciding to work harder.
- **PM provider label removal** on project promotion. The abstraction has no
  remove-label op; promotion records residual `project:<slug>` labels instead.
  Provider-level API work, not promotion's job.

## Changes

### 1. `lf loop` — inhabiting is a call, not a mode

Inhabiting is the wave LLM choosing to run `lf loop project "..."` and wait.
Nothing about the wave changes: `spawn_wave_pass` stays hard-coded `"wave"`,
the wave scheduler never reads `scratch/loop.yaml`, there is no flow field and
no lease machinery. The bit lives in the loop's own worktree, where the driver
already reads it; caps, `recheck`, and escalation run under the normal driver
with a wave for a parent — exactly as today. (An earlier draft swapped the
wave's own flow instead. This answer deletes that entire mechanism.)

Entering a task or project loop always forks a worktree, so the wave home
never lands and `lf pr land`'s `scratch/*` sweep never touches it.

**Blocking is a long tool call.** While `lf loop` runs foreground, the wave's
pass is inside one tool invocation: the human attached to the wave session gets
no reply until it returns. Steering still lands — the loop's children re-read
the wave's thread at *their* pass boundaries — so blocking trades the thread's
tool-boundary ear for the inner loop's pass-boundary ear. Say it in the pursue
skill so waves block knowingly.

**A running child extends the lease.** `pass_timeout: 1800s` would kill a
blocked pass a quarter of the way through a 2h loop. The rule generalizes from
the human case: presence extends the lease, and a live child loop is presence.

**Detached loops are server-owned.** `lf loop task "..." --detach` asks the
wave server to spawn and supervise the loop — the server is the long-lived
process, so it is the right parent, and it can park the loop in a named session
for read-only attach. Plain `&` half-works today (the orphan re-parents, its
store row completes) and *keeping it working* is the simplicity tripwire: the
moment shell backgrounding can't survive, the loop has grown too entangled with
its parent pass. `--detach` adds ownership, not a requirement.

**`--dispatch` should go.** `lf loop <flow> --detach` covers it. That means
rewriting `wave_exec_verdict`, which keys its whole allowlist on the flag:
*"any flow / inline prompt WITHOUT `--dispatch`"* is the deny arm keeping a
leaked subagent token from running an arbitrary LLM prompt unsandboxed in the
outwave. **That security property must survive the rename.**

**The detached-loop contract: write where the wave reads.** Memory accumulates
at the wave level — a hand has none of its own — so a detached loop is only as
real as its writes to the wave's surfaces:

- **PRs** — the record of done; the merge is the bit.
- **Reports** (`<self>/report`) — progress and completion through the wave chat
  system; these wake the wave.
- **`lf memory add`** — learnings land in the wave's memory as they happen,
  not in a transcript that dies with the worktree.

A weeks-scale project loop that is merging PRs, reporting at boundaries, and
recording learnings is fully legible from the wave's thread without anyone
reading its transcript. One that writes to none of these is invisible, and
invisible work is failed work — the loop skills should say so.

### 2. Memory walks the chain; chat does not (`wave_context.rs`)

`<lf:wave-memory>` walks `parent_wave_id`. `<lf:wave-chat-recent>` stops at the
wave. One function assembles both today.

### 3. `lf project promote <slug>` — a flow

Mechanical: `wave/<parent>/projects/<slug>.md` → `wave/<slug>/GOAL.md`; set
`parent_wave_id`; publish `.wave-endpoint`; start residency. PM: the subwave
needs its own Linear project and the `project:<slug>` labeled tasks migrate —
`lf pm sync --plan` already reports that drift.

Authored, not moved: a project has no `crons`, no `workers`, no budget.
Promotion must *invent* a cadence. That is a judgment, which is why this is a
flow with a skill and not a shell op.

Its bit is clean: the subwave's first pass posts to its own thread and the
parent hears it via `--parent`. A real-world condition, checkable, not
self-report.

Under the scope chain, promotion is reversible: demotion folds the child's
`MEMORY.md` into the parent's and drops the residency. Without it, promotion is
a ratchet — two copies of every shared fact, one of which rots.

### 4. Close the tmux door

`tmux attach -r` is read-only and keeps the debugging window. What stdin is
currently load-bearing on: LOOPFLOW.md advertises attach to *"answer an
interactive skill."*

The loop already solved this and execs did not inherit it —
`run_pass(content, answers, inbox_rx)` (`wave.rs:439`) threads answers from the
inbox. So closing the door is either porting that path to execs, or declaring
dispatched execs non-interactive by construction. They already run `lf -b`,
headless, so the second may be nearly true today and merely undocumented.
**Check this before writing the rule down** — it decides whether "no direct
control" is a doctrine change or a feature.

### 5. Backlogs are allowed

Agents may file tasks without running them. No enforcement that a task be solved
to be added.

This resolves the standing contradiction — `loop/README.md:35` (*"No
backlog… intent that isn't running yet lives in GOAL.md, memory, and chat — not
in a tracker"*) versus `LOOPFLOW.md` (*"Concrete tasks live in Linear"*) — in
Linear's favor. The loop README becomes wrong, not merely in tension, and
must be rewritten.

Be deliberate about what "no backlog" was buying. It made *"the open runs ARE
the wave's open tasks"* literally true, so `lf runs` and the `<in_flight>` fold
were a complete picture of intent. A task now has three states, not two: filed,
running, merged. The wave must **read** its backlog to select — `wave_pursue.md`
already reads live Linear tasks, so the plumbing exists. What "no backlog"
prevented was a tracker filling with intent nobody will do, and nothing else in
this design prevents it.

### 6. Mac surface

Each of these needs an MVP in Loopflow Mac, alongside Goals and Projects. The
wave is the only mind, so the wave is the only screen — everything below is a
pane on it, not a place to navigate to.

- **KRs** — every project's KRs for this wave, with their proof state. The bets,
  and whether they hold.
- **Open PRs** — the record of done-and-pending. Under no-backlog these *were*
  half the task model; they still are the closure evidence.
- **Active sessions** — the hands. Which loops are running, foreground or
  backgrounded, in which worktree, at which pass. This is the `<in_flight>` fold
  and `lf runs`, surfaced.
- **Backlog** — filed-but-not-running tasks, now that they exist.
- **Thread** — the one chat: the journal-backed fold, live-attached to the
  running pass's session when one exists. Reports surface here; subwaves do
  not. The phase currently holding the pen (clarify / pursue / mutate) can show
  as presence.

The design constraint is the one from the model: a session is not a
conversation. Rendering each active session as its own chat is precisely how
this design gets undone in the UI after being right in the runtime.

### 7. Doctrine

`wave_pursue.md`'s frontmatter reads *"Delegate wave work through project and
task loops."* Its escape clause — *"Run execs directly only for hot, now
problems"* — is the line this design deletes.

Because the three pursue skills sit in the same slot, "inhabit one, delegate the
rest" is one sentence that recurses for free. A wave keeps one project and
delegates the others; the kept project keeps one task and delegates the others;
the kept task is a PR. A single-project single-task wave is one thread producing
one PR, with no special case for it.

The selection criterion is *not* importance. Inhabited work advances at wake
cadence; delegated work advances continuously. Keeping the most important
project would systematically starve the priority. Keep the one whose next move
needs the wave's memory and chat in the room — the one you could not write a
self-sufficient seed for. The delegation test and the seed test are the same
test.

### 8. Channels are wires, not records (this branch)

A journal buys exactly one thing over a broadcast bus: delivery to a subscriber
who wasn't listening at publish time. Minds need that (their loop queues
between passes, their thread replays on reconnect); topics never do. So
journaling is a property of minds, not channels: publishers journal nothing,
and a subscriber records what it needs to remember in its own journal.

The bus gets its own verb: **`lf radio`**. Agent-to-agent only — hands report
up, minds steer down, `lf sub` tunes in. The name carries the contract for
free: radio is broadcast, not delivery — you transmit, whoever's tuned in
hears it, nobody guarantees receipt; miss it and it's gone. Channels were
already radio vocabulary before the medium had a name, and a server-stamped
byline is exactly how radio speech works ("this is goals.148e reporting").

`lf chat` narrows to what the word actually means: a human conversing with a
**served mind** — journaled, durable, replayed, because that surface is the
product. Two words, two wires. The old `chat` was one verb fused across both,
and that fusion is where every steering confusion in this design traced back
to.

Chat and serve stay separate commands, deliberately — the same cut as
serve/loop. `lf serve` is lifecycle: boot the mind, own the process, outlive
any one conversation. `lf chat` is attachment: the human client to an
already-served mind — it streams the thread, sends what you type into it, and
boots nothing (no live server → the `lf serve <wave>` error). Quitting a
conversation must never kill a mind. postgres and psql, not one verb wearing
two hats.

The three verbs, complete:

| Verb | Who uses it | What it is |
| --- | --- | --- |
| `lf serve <wave>` | human (or promotion) | boot a mind; own its process |
| `lf chat` | humans only | converse with a served mind's thread |
| `lf radio` | agents only | broadcast on the bus; ephemeral |

The model, in full:

- A channel is a name on the broadcast bus. Anyone may publish to any channel;
  anyone may subscribe to any channel or prefix. No journal, no worktree
  binding, no liveness precondition. A message published with no subscriber
  listening is gone, and that is correct.
- Attribution is the security boundary, not permission: the server stamps who
  spoke from the caller's token. A client-supplied byline cannot spoof another
  speaker. (This replaces the earlier prefix write-gate sketch — open publish
  with honest bylines, rather than routing rules.)
- The default conversation shape: launch a hand, listen on its channel, respond
  into its channel when it needs steering. The wave's listener is always
  connected, so everything it hears from its hands that matters lands in the
  wave's own journal, attributed — that is today's "fold upward," which was
  always really the parent's subscription.
- A detached loop's driver holds its subscription for the loop's lifetime and
  queues in memory. A queue that dies with its loop is honest; the slow path
  (the hand re-reads the wave's thread each pass boundary) exists regardless.

What deletes: `ChildChannel` and the per-worktree journal files, the
consumption-local fold and its tests, the worktree-liveness dance ("channel has
no live worktree"), the FLAGGED archive fallback, `lf chat`'s channel
addressing and `--parent` transport (both become `lf radio`), and
`channel.rs`'s definition of a channel as "journal + thread + subscribability"
— it becomes name mechanics and broadcast only.

#### Done when: no journal exists outside a served mind

- The only journal files on disk live at `.lf/journal/waves/<wave>/` under the
  origin repo — one per served mind. Running a detached loop end-to-end leaves
  no journal in its worktree; `child_worktree_path` no longer exists in the
  tree.
- The deleted machinery's tests are deleted with it, not skipped. Full suite
  green.

#### Done when: two agents converse over a channel, live

The demo, concretely — what the developer runs and sees:

```bash
lf serve goals                       # terminal 1: the mind
lf loop task "fix the flaky test" --wave goals --detach   # terminal 2: a hand
lf sub goals.<run>                   # terminal 2: watch the hand narrate, live
lf radio --channel goals.<run> "skip the migration, just gate it"  # steer it
```

- The hand's report (`lf radio` from inside its worktree — bare, it publishes
  on its own channel) appears in the wave's thread attributed `[goals.<run>]`,
  queued for the loop — one copy, in the wave's journal, nowhere else.
- ~~The steer reaches the hand: its driver's live subscription picks it up.~~
  **Did not land as written** — the driver holds no subscription, so the
  broadcast reaches `lf sub` listeners only; steering a hand means speaking on
  the wave's thread (the ear it reliably has). §9 reopens the fast path as a
  cheap poll cursor.
- A message published to a channel nobody is listening on writes no file
  anywhere.
- The byline on every frame is server-stamped *from the channel* (one token
  per boot means the server can't tell hands apart); a forged `from` does not
  survive. §9 supersedes this with client-submitted bylines — see its
  testimony/evidence done-when.

#### Done when: LOOPFLOW.md teaches radio as agent-to-agent comms

- LOOPFLOW.md states the contract in one short block: `lf radio` is agents
  talking to agents — report up when you finish, fail, or get stuck; broadcast,
  not delivery; not a log, not a notebook, and not the human surface. `lf chat`
  is the human's conversation with a served mind, and agents don't use it.
- The listen/respond pattern (launch a hand → subscribe to its channel →
  respond into it) rides in the loop skills that launch hands, not in every
  context — doctrine only where it's exercised.
- Nothing in the builtins still describes channel journals, worktree-bound
  records, `chat` as an agent verb, or `--parent` as a special transport.

### 9. The bus is the store, not the listener

§8 split the wires conceptually and left the bus living inside the thread's
process. Trace what `lf radio` actually does at HEAD: resolve the family
head's endpoint, `POST /messages` to the mind's listener, which sends one
tagged frame on `family_tx` — a tokio broadcast channel in the listener's
memory — and, for a `say`, folds one copy into the wave's journal. So there is
no bus; there is a broker, and the broker is the mind. "Anyone can publish"
means "anyone can RPC the one process that owns this family, if it is
running." Two detached hands cannot hear each other unless their wave is
awake. A human running batch loops with no served wave is deaf. "Tuned in"
means "holding an open SSE socket at that instant" — ephemerality is an
implementation accident, not a designed property.

The trace also shows radio is two primitives wearing one verb. *Narration*:
ephemeral fan-out, no guarantee — genuinely radio. *Report*: a durable write
that wakes one named consumer — a mailbox, not radio. They want different
things, and neither wants a broker.

The design: **the db IS the bus**, the same move that made the db the
registry. One table in the shared store — `{rowid, channel, byline, text,
at}`. Publish is an INSERT; no server is in the path, so publishing works
with zero loopflow processes running. Subscribe is a forward poll from a
rowid cursor: you hear what is said while you listen, from where you tuned
in. A sweeper deletes rows past a wall-clock window — the bus is not a log,
and the temptation to treat it as one is the failure mode to guard.

What falls out:

- **Byline is testimony; channel is evidence — structurally.** With no server
  in the path, client-submitted attribution is the only kind possible. The
  client derives its byline from the ambient identity it already resolves for
  routing (`LFD_CHANNEL` → `LFD_WAVE_ID` → worktree name) and writes it into
  the row next to the channel. A forged byline is visible as a mismatch in
  the record, not prevented. This deletes both §8 gaps at once: no per-hand
  tokens (nothing stamps), and no steering-direction hole (a mind speaking on
  a hand's channel is bylined as the mind).
- **The listener demotes to a subscriber.** It polls the bus like anyone,
  records reports addressed to its family into its own journal (the §8 fold,
  now reading from a table instead of its own arm), and wakes its loop. Its
  cursor is durable, so a mind that was asleep when a hand reported catches
  up on wake — the sleeping-mind hole closes, within the sweep window. A mind
  asleep longer than the window misses the report; the PR and run ledger
  remain the records of record. Disclosed, not hidden.
- **A hand's ear is a cursor too.** The detached driver polls its own channel
  between passes — the "live subscription" §8 imagined becomes a poll loop it
  can actually hold, or stays unbuilt with the wave's thread as the one ear.
  That fork stays open; the bus makes both cheap.
- **The deletion list grows again**: `family_tx`, `ChannelFrame`, tagged SSE
  frames, the `?channel=`/`?prefix=` scopes on `/events`, the `channel` field
  on `POST /messages`, `deliver_to_channel`, radio's endpoint resolution, and
  the `--steer` flag on radio (steer is a thread op; the shared dispatch let
  it leak across the verb split — separate transports make the leak
  unspellable). `POST /messages` becomes purely the thread door. Chat and the
  Mac are untouched: the thread stays SSE on the listener.
- **The same table pattern is the one-writer answer.** One-brain-per-wave is
  already a pid-probed store row; a worktree lease is the identical mechanism
  one table over. Four writers hit this branch's worktree in one day on luck
  and vigilance. The db IS the registry, the db IS the bus, the db IS the
  lock.

#### Done when: publishing needs no server

**Landed.** Verified against the built binary with an isolated `LF_HOME`, no
`lf serve` anywhere: `lf sub ship` tuned in first, then two hands exchanged
messages on each other's channels and both lines printed within a poll interval
(250 ms). A broadcast on the out-of-prefix channel `other` was not heard. The
sweeper: a row aged past the 1 h window vanished on the next publish, and the id
kept climbing (AUTOINCREMENT, so no cursor ever rewinds). Size: 1440 realistic
reports — a full day at one a minute — is 127 KB of content; only the ~60 inside
the window are ever resident.

- With zero loopflow processes running, `lf radio "note"` exits 0 and the row
  is in the bus table. `lf sub <channel>` in another terminal, started first,
  prints it within a poll interval. No HTTP anywhere in the path.
- Two detached hands exchange messages with no served wave.
- A row older than the sweep window is gone, and the bus table stays small
  under a day of real use — measured, not assumed.

#### Done when: a sleeping mind catches up

**Landed.** `wave::bus::BusListener` holds the durable cursor (`bus_cursors`,
keyed by the wave's channel name). Tests:
`a_sleeping_mind_catches_up_exactly_once` boots, kills, publishes with the mind
down, restarts twice, and finds one copy;
`a_swept_report_leaves_a_visible_cursor_jump` sweeps two unread frames and finds
the note `bus cursor jumped 0 → 3: 2 broadcast(s) aged past the sweep window` in
the thread ahead of the surviving report. One rule added beyond the design: a
mind skips rows bylined with its own channel, so steering a hand never wakes the
steerer.

- Serve a wave, kill it, have a hand `lf radio` a report, restart the wave:
  the report lands in its thread, attributed, exactly once — the cursor, not
  luck, decides. Repeat the restart; still exactly once. (The floor is
  at-least-once: the journal write and the cursor commit are not one
  transaction, so a crash in that seam replays the row. See
  `scratch/questions.md`.)
- A report published beyond the sweep window is missed and the miss is
  visible (the journal shows the cursor jump), not silent — including when the
  sweep left the bus empty and nobody published after it.

#### Done when: byline is testimony, channel is evidence

**Landed.** `lf radio --from ci -c ship.a "all green"` from inside `ship.a`
prints `[ship.a] ci: all green` on a subscriber: byline `ci`, arrival channel
`ship.a`, mismatch in the record. Nothing derives identity server-side —
`radio.rs` writes the byline the client resolved, and the runtime's
channel-stamping path is deleted.

- Every row carries the client-submitted byline and the arrival channel; the
  reader can see both. No code path derives identity server-side.
- `lf radio --from ci` on a hand's channel produces a row whose byline says
  `ci` and whose channel says the hand — the mismatch is in the record.

#### Done when: the broker is gone

**Landed.** Each of `family_tx`, `ChannelFrame`, `tagged_turn_json`,
`deliver_to_channel`, `subscribe_channels` greps to zero across Rust, Swift, and
Python; `/events` has no scope query and `/messages` has no `channel` field
(only `POST /channels`, §8's dispatch knock, still spells the word).
`lf radio --steer` is `error: unexpected argument '--steer' found`. `radio.rs`
imports `wave::channel` and `wave::runtime` for name mechanics and nothing from
`wave::server`. `lf sub` no longer opens a socket at all; the SSE follower it
used to own now lives in `lf/commands/thread.rs` and backs `lf wavechat`.

- `family_tx`, `ChannelFrame`, tagged `/events` frames, `?channel=`/`?prefix=`
  scopes, the `channel` field on `/messages`, and radio's endpoint resolution
  no longer exist in the tree. Radio compiles with no dependency on
  `wave::server`.
- `lf radio --steer` is a parse error, not a forward to chat.
- LOOPFLOW.md's radio block needs no "if the wave is running" caveat, because
  there isn't one.

## MVP product proof

The MVP is one local wave with one continuous Chat thread, one persistent
playhead, and explicit enqueue and skip controls. It proves the mind can move
through default, nested, and inserted flows while the user experiences one
conversation and can always tell where it is.

Promotion, demotion, remote execution, detached-hand UX, the complete KRs/PRs/
backlog dashboard, and a full editable flow graph are not required for this
proof. They remain consequences of the architecture, not prerequisites for
trying the core interaction.

### Done when: the mind is always somewhere

- Starting a wave opens one Chat thread whose header names the active flow and
  step without requiring the user to find or choose a session.
- The header shows a breadcrumb for the invocation stack plus `now` and `next`.
- When one body ends and the next starts, the playhead transitions in place;
  the Chat never resets or presents an empty "no session" state.

### Done when: the default flow visibly loops

- A complete `wave` cycle advances through clarify, pursue, and mutate, then
  wraps to clarify again.
- The wrap starts a new invocation/cycle in the same thread. Completed turns
  and their body boundaries remain above it.
- With nothing explicitly queued, the resident continues its default cycle
  without a human relaunch.

### Done when: chat reaches the body now playing

*Met from the Mac and `lf chat --steer` with Codex. Claude and OpenCode queue
to the next body because their harnesses do not expose live steer.*

- Sending a message while a step is active delivers it into that step's live
  session rather than waiting for the whole flow to finish.
- The reply streams beneath the active body boundary and journals as it grows.
- A message and its answer appear exactly once after reconnect or replay.

### Done when: a flow can be enqueued at the current scope

- From `wave › review-design › pursue`, enqueueing `research` shows it in
  the `review-design` invocation's local queue and shows the return target in
  `wave`.
- `review-design` finishes its remaining steps, `research` runs, and only then
  does the playhead return to the suspended `wave` invocation.
- The inserted flow uses the same thread and memory. If its invocation needs a
  placed worktree, the placement changes without creating another chat.
- Enqueueing two flows at the same scope runs them FIFO before returning to the
  caller.

### Done when: skip advances without destroying the route

- Pressing Skip interrupts the active session, records a visible `skipped by
  user` boundary, and starts the next step.
- Skipping preserves the current invocation's remaining steps, local queue,
  and return target.
- Output arriving from the skipped session after the boundary cannot append to
  the new body's span.

### Done when: session handoffs are legible but secondary

- Every assistant span sits beneath a boundary labeled with flow and step.
- Expanding a boundary reveals invocation id, step path, session id, harness,
  model, host, worktree/run, start/end time, and termination reason.
- Collapsing the details leaves a readable conversation. There is still one
  composer and no session-shaped chat tabs.

### Done when: nested navigation fits in the Chat header

- During a nested flow, the primary UI answers four questions without opening
  an inspector: where am I, what is next, what is queued here, and where do I
  return?
- Repeated invocations of the same named flow remain distinguishable; the UI
  and journal key them by invocation id and step path, not display name.
- Loops show their current iteration and branches show the selected path. The
  unchosen graph stays in the audit view rather than crowding Chat.

### Done when: the playhead survives reconnect and restart

- Closing and reopening Loopflow Mac reconstructs the same thread grouping,
  invocation stack, current step, local queues, and return targets.
- Restarting the wave server marks the abandoned body interrupted, resumes the
  same logical step in a new session, and preserves every queued continuation.
- Recovery neither silently advances a step nor duplicates a completed turn.

### Done when: a failed body does not become a lost mind

- Killing the active harness produces a visible failure boundary tied to that
  body.
- The scheduler retries the same logical step in a new body; it does not mark
  the step complete or advance the playhead because a process exited.
- If the step cannot restart, Chat remains on that failed step with Skip
  available. The user never has to attach to tmux to recover control.

### Done when: the record explains the experience

- Replaying the journal reproduces the same thread spans, body boundaries,
  playhead, queue, skips, failures, and resumptions shown live.
- Selecting any assistant span opens the run/session trace that produced it.
- A tester can inspect the replay and answer which body produced every piece
  of assistant output and why the playhead moved next.

### The MVP demo: enqueue, skip, return

1. Open a running wave in Chat and watch its default flow advance without
   selecting a session.
2. Enqueue `review-design`; while inside it, enqueue `research`.
3. Send a message and watch the active body answer inline.
4. Skip one step. See the skip boundary, the next body, the queued `research`
   flow, and the eventual return to the suspended wave invocation.
5. Close and reopen the app. The thread, body boundaries, playhead, queue, and
   return point are unchanged.

The MVP holds when that entire demonstration uses one Chat thread and never
requires a terminal attachment or session picker.

## Demo

```bash
lf loop infrastructure          # a wave with three projects, one chat
lf chat --steer "how's release stability?"
```

The wave answers from its own memory. It has inhabited `technical-architecture`
— the project whose next move needed the thread — and drives it with
`lf loop task "..."`, blocking, in a forked worktree, to a merged PR.
`developer-efficiency` and `release-stability` run as detached loops,
server-owned, reporting up. `lf runs` shows hands; nothing there is a
conversation.

Steer without leaving the thread:

```bash
lf chat --steer "actually skip the migration, just gate it"
```

If a pass is live, the message lands inside its running session at the next
tool boundary — seconds, not a pass boundary. If not, it journals and the pass
it wakes is born already caught up, because every mind receives recent chat at
birth.

Open Loopflow Mac on the same wave: one screen, the thread plus KRs, open PRs,
and active sessions. The screen has no second conversation on it.

```bash
lf project promote release-stability
lf chat --wave release-stability --steer "drop the flaky retry, delete the test"
```

A second room, because you asked for one. The parent stops overhearing.

## Resolved along the way

- **Buffer vs byline** was never a choice: the bus carries narration and
  reports; the thread is not a topic at all.
- **A separate chat LLM vs the loop** was a false pair: the chat is an
  attachment mode on the running pass. One head; the thread outlives its
  bodies.
- **Does a loop child re-read the wave live?** Yes, verified: no `env_clear`
  in the pass-spawn path, and `LoopRun` worktrees are wave-named
  (`<repo>.<wave>.<run-id>`), so ambient resolution lands on the wave with or
  without `LFD_WAVE_ID`. The ear is at pass granularity.
- **The finish-wake hole**: done is a report on `<self>/report`; the loop
  subscribes to reports, not narration. A hand that finishes silently never
  reported — legible as its failure.
- **Inhabitation is a call, not a mode.** The wave LLM chooses to run
  `lf loop project ...` and wait. No flow-swap, no lease machinery, no bit in
  the wave home; the earlier draft's whole §1 mechanism deleted itself.
- **Detached loops are server-owned.** `--detach` over `&` — but `&` continuing
  to half-work is kept as the bash-simplicity tripwire: the day shell
  backgrounding can't survive, the loop has grown too entangled with its
  parent.
- **Project loops exist** — `lf loop project ...` is the inhabitation verb
  itself. The weeks-vs-`max_passes` tension is real but the memory concern
  is not: hands hold `lf memory add`, so learnings land in the wave as they
  happen.

## Open questions

**Caps for a project loop.** `max_passes: 8` and `wall_clock: 7200s` fit a
task's merge horizon, not a project's KR horizon. Per-flow defaults, or
per-invocation overrides in the pursue skill's vocabulary.

**Topic vocabulary and retention.** `<self>` and `<self>/report` fall out of
the design; whether the Mac panes (PRs, runs, KRs) are topics or store queries
decides whether the UI is a subscriber or a poller. Narration topics can die
with their worktrees; the thread's journal must not. Notation: pick dots or
slashes — `matches_prefix` speaks dots.

**Can a hand grow hands?** The namespace tolerates `goals.a.b`. *Only minds
delegate* is the tighter answer.

**Depth.** Two levels is probably the honest limit — a grandchild's news
reaches the root only if its parent re-authors it, and nothing structural says
so.

**"Delegate all but one" is still procedural, at arity N−1.** Two projects
contending on the same files should not both run, and the real parallelism
unit is in-flight runs, not projects. *Parallelism degree equals project
count* is a legible invariant, and legibility may beat optimality for
something a human steers — but it is a choice, not a consequence of this
design.
