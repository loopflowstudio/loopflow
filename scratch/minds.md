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

**Writes go down. Reads go up.** Publish is permitted iff
`matches_prefix(target, writer_channel)` — your channel and its subtree — with
`writer_channel` derived server-side from the token. A hand narrates to
`<self>` and authors reports to `<self>/report`; the parent *subscribes* to
reports and not narration. A child wave likewise. No upward write exists
anywhere. The prefix rule is total, with zero exceptions — `lf chat --parent`
becomes sugar for `--channel <self>/report`, and `driver.rs:206` stops being a
crossing: the child was always talking in its own room, and the parent was
always the one who chose to listen.

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
many processes post to it. Journals move to the origin; retention becomes
per-topic policy rather than a side effect of branch deletion. The FLAGGED
archive note (`~/.lf/journal/<repo>/<worktree>`) stops being a fallback and
becomes the design.

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

**Built and verified.** Full suite green (`cargo test`, all targets).

| Change | Where |
| --- | --- |
| §1 `lf loop` — inhabit, delegate, `--detach`; `--dispatch` deleted | `bin/lf.rs`, `flowloop/driver.rs`, `wave/server.rs` |
| Exec door pins a detached loop to its own wave | `wave/server.rs:774` |
| §2 Memory walks the parent chain; chat stays local | `engine/wave_context.rs` |
| §3 `lf project promote` — flow, skill, parent link, PM move | `ops/project.rs` |
| The playhead: model, FIFO nested frames, skip, retry-on-failure | `wave/playhead.rs` |
| Playhead durability across reconnect and restart | `wave/journal.rs`, `wave/runtime.rs` |
| §4 tmux door read-only · §5 backlogs allowed · §7 doctrine | `engine/builtins/**` |
| §6 Mac surface — five panes, playhead header, body provenance | `swift/LoopflowMac/Views/**` |

**Not built.** Each is a real gap, not a rough edge.

| Gap | Consequence today |
| --- | --- |
| CLI `lf chat` never steers — it always posts `op:"say"` (`lf/commands/chat.rs:95`) | The **Demo** below overclaims: only the Mac composer emits `op=steer`, and only Codex consumes it (`flowloop/wave.rs:727`). A CLI message queues for the next pass. |
| The bus/thread rewrite: channels-as-topics, writes-down/reads-up, server-stamped attribution | Chat still carries client-supplied `from`; no write-prefix gate exists. Root cause of the row above. |
| Mid-turn steer for Claude and OpenCode → *roadmap* | Only Codex steers; the others queue to the next body. Vendor-gated; the product question lives in `wave/product/projects/wave-chat.md`. |
| Composite flow nodes (and/xor/or/loop) as first-class playhead frames | They run through the internal headless `__flow-step` fallback (`flowloop/wave.rs:311`). |
| PM `project:<slug>` label removal on promotion → *roadmap* | The provider has no remove-label op; residual labels are recorded, not cleared. |
| Project-loop caps | `lf loop project` still inherits the generic 8-pass / 2-hour defaults (`flowloop/driver.rs:44`). |
| Foreground/background label in Active sessions | The run ledger doesn't persist the owner, so the Mac shows pass/worktree/liveness and declines to guess. |
| One writer per wave worktree | Nothing enforces it. This branch was written by two agents at once and it left a self-contradicting test in the source. |

**Fixed after the fact** — defects this branch shipped and a later session repaired:

- `lf project promote` spawned `lf wave <child>`, a subcommand this same branch
  deleted. Promotion could never start residency. Now `lf loop <child>`.
- Three wave tests asserted pre-rename wire shapes (`"thinking\nmore"` for what
  is now exact stream concatenation; `{"id":null,"op":"interrupt"}` for what is
  now `{"kind":"interrupt"}`).
- Two `bin/lf.rs` tests still named the deleted `wave` command. They had never
  run: a failing lib target makes `cargo test` skip every later target, so the
  lib failures masked them. **Run `cargo test` to completion before trusting a
  green-looking suite.**

## Next implementation session

Work that is schedulable now, ordered by what unblocks the most. Everything here
is ours to write; nothing waits on anyone else.

1. **Make `lf chat` steer.** The mechanism exists and the Mac uses it. Give the
   CLI an op — infer `steer` when a turn is live, or take it as a flag — so the
   headline Demo becomes true. Smallest change that closes the widest gap
   between doc and product.
2. **The bus/thread wires** (§"Two wires"). Server-stamped attribution and a
   `matches_prefix` write gate. Settles the topic notation below and subsumes
   (1)'s root cause.
3. **Project-loop caps.** Needs dogfood data, not a guessed weeks-scale timeout.
   Run one real project loop first, then pick.
4. **Composite playhead frames.** Promote branch/loop internals out of the
   `__flow-step` fallback. Separate graph-runtime work; do it when the Mac's
   breadcrumb starts lying about nested flows.
5. **One writer per worktree.** Decide whether it's a wave-home invariant or a
   lock. Until then, check for a live agent before working a wave worktree.

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

*Met on the Mac, with Codex. Not met from the CLI, and not for Claude or
OpenCode — see **Status**.*

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
lf chat "how's release stability?"
```

The wave answers from its own memory. It has inhabited `technical-architecture`
— the project whose next move needed the thread — and drives it with
`lf loop task "..."`, blocking, in a forked worktree, to a merged PR.
`developer-efficiency` and `release-stability` run as detached loops,
server-owned, reporting up. `lf runs` shows hands; nothing there is a
conversation.

Steer without leaving the thread:

```bash
lf chat "actually skip the migration, just gate it"
```

If a pass is live, the message lands inside its running session at the next
tool boundary — seconds, not a pass boundary. If not, it journals and the pass
it wakes is born already caught up, because every mind receives recent chat at
birth.

> **Not true yet.** `lf chat` always posts `op:"say"`, which queues for the next
> pass. Only the Mac composer emits `op=steer`, and only Codex consumes it. This
> is item (1) in **Next implementation session** — the widest gap between this
> doc and the product.

Open Loopflow Mac on the same wave: one screen, the thread plus KRs, open PRs,
and active sessions. The screen has no second conversation on it.

```bash
lf project promote release-stability
lf chat --wave release-stability "drop the flaky retry, delete the test"
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
