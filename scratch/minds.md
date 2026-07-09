# Minds: inhabit, delegate, promote

## Problem

A wave that wants work done has exactly one move: dispatch. `wave_pursue.md`
says *"Launch project or task flowloops for clear planned work"* and allows
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

| Verb | Whose mind | Whose hands | Cost |
|---|---|---|---|
| **Inhabit** | mine | mine | serialization; the work's turns spend my context budget |
| **Delegate** | mine | other | a private transcript — every learning that stayed inside it |
| **Promote** | other | other | residency: a process, an endpoint, a cadence, a budget |

There is no fourth cell. "Other mind, my hands" does not exist.

**Inhabit** is the default. A wave with one project *is* that project for the
duration. `spawn_wave_pass` hard-codes `.arg("flow").arg("wave")`; that string
becomes the flow the wave is currently inhabiting.

**Delegate** when the work neither needs your memory nor should change it. The
first half is the seed test `project_pursue.md` already states: *"The seed is
the task's whole handoff — make it computable on its own."* The second half is
context hygiene — delegation is a deliberate refusal to let the work's detail
into your read set. That is the real content of the transcript-bloat argument,
which turns out to be principled rather than incidental.

Parallelism sits outside both. It is an orthogonal reason to fork: sometimes
you would rather inhabit and cannot, because two things must move at once. Then
you pay the transcript cost whether you wanted to or not, and the summary-back
is the repair.

**Promote** when you need to steer it while it runs.

### Waves are the only minds

`wave/channel.rs` already says this:

> Child channels have NO flowloop: they are pure streams (no `FlowloopState`,
> no memory — a work line's notes are files; **MEMORY.md is wave identity**).

There is no way to launch a mind without promoting to a wave. Task and project
flowloops are runnable *from within* a mind:

```bash
lf loop task "..."      # block on solve — the wave's own pass drives it
lf loop task "..." &    # background solve — concurrency, the only reason to
```

Entering a task or project flowloop inherently forks a worktree. That is not a
cost to weigh; it is what the verb means.

### Two wires: the bus and the thread

The `say` op is two different things fused. Today it is both agent-to-agent
signaling (wakes the flowloop, coalesces into the inbox) and the human chat
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

**Wake is subscription.** The flowloop wakes on reports, heartbeat, and cron.
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
unchanged — restart-the-flowloop is a steering lever that already exists and is
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

### What a hand is

A hand has a voice and no room. Its transcript is private — *"Trust worker
summaries; do not reread worker transcripts."* Its posts are public. What forks
under delegation is neither memory nor voice; **it is the transcript**.

A hand's ear is subtler than it looks. A one-shot `--dispatch` exec is seeded
once and deaf after. A **flowloop** is many births: every pass is a fresh
process inheriting `LFD_WAVE_ID`, and context assembly resolves the *wave* from
it. So a flowloop re-reads the wave's live memory and thread at every pass
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

## Changes

### 1. The wave inhabits (`flowloop/wave.rs`)

`spawn_wave_pass` hard-codes `.arg("flow").arg("wave")` (`wave.rs:643`). Make it
the flow the wave is currently inhabiting; default `wave`.

The hierarchy level is exactly one function call. `run_flowloop`
(`driver.rs:83`) does two separable things:

```rust
let run = FlowloopRun::start(&wave_name, &options.flow, seed)?;   // mints worktree + row + channel
let result = drive(&worktree, seed, options, |flow, seed, opts| { // the loop
    run_pass(&worktree, flow, seed, opts)
});
```

`drive()` needs no run row, no channel, no store. Inhabiting means looping
without minting the run.

The wave's scheduler must learn to read `scratch/loop.yaml`, which it currently
ignores, and interpret `done: true` as *lease over, revert to `wave`* rather
than *loop over*.

Three things do not transfer:

- **Caps are written for a tight loop.** `max_passes: 8`, `wall_clock: 7200s`
  assume back-to-back passes. An inhabited loop advances one pass per wake
  (heartbeat is 4h), so eight passes is a week and the wall clock fires after
  two hours having done one. Caps must be lease-relative or dropped.
- **Exhaustion escalates to a parent the root wave does not have.**
  `loop_instruction` promises it; `chat.rs` says a root wave errors on
  `--parent`. Unreachable today because waves have no bit. Reachable the moment
  one reads a bit.
- **`recheck` is a third wake source, free.** `drive()` polls the predicate
  between passes; the wave's scheduler has inbox and heartbeat and no notion of
  *the world changed*. An inhabited task whose bit is
  `gh pr view … | grep -q MERGED` wants exactly that wake, and it is what
  closes the cadence gap against a dispatched loop.

### 2. `lf loop` — a hand, foreground or backgrounded

Entering a task or project flowloop always forks a worktree. So the wave home
never lands, `lf pr land`'s `scratch/*` sweep never runs in the outwave, and
`scratch/loop.yaml` is safe there as the inhabited loop's bit. The rule protects
itself.

`lf loop task "..."` blocks: the wave's pass drives it. `lf loop task "..." &`
backgrounds it — concurrency, which is the only reason to.

Foreground is where the caps collide. A wave pass has `pass_timeout: 1800s`; a
task loop has `wall_clock: 7200s`. Blocking, the hand outlives its parent pass
and is killed. Either the caps invert, or the wave pass *is* the loop's driver.
And while blocked, the wave cannot answer chat — the inbox coalesces to a
boundary two hours out. Unless the ear is at pass granularity, in which case it
did not close, it **moved**: your message reaches the task pass, which is the
thing actually working. Blocking means the wave lends its ear to its hand.

Background needs an owner. `spawn_wave_pass` sets `kill_on_drop(true)`, but a
shell-backgrounded grandchild survives its parent. `FlowloopRun::start` mints a
store row so it shows in `lf runs` and the `<in_flight>` fold — nothing reaps
the process. `--dispatch` at least parked it in a named tmux session.

If `lf loop <flow> &` covers everything `--dispatch` did, `--dispatch` should
go. That means rewriting `wave_exec_verdict`, which keys its whole allowlist on
the flag: *"any flow / inline prompt WITHOUT `--dispatch`"* is the deny arm
keeping a leaked subagent token from running an arbitrary LLM prompt unsandboxed
in the outwave. **That security property must survive the rename.**

### 3. Memory walks the chain; chat does not (`wave_context.rs`)

`<lf:wave-memory>` walks `parent_wave_id`. `<lf:wave-chat-recent>` stops at the
wave. One function assembles both today.

### 4. `lf project promote <slug>` — a flow

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

### 5. Close the tmux door

`tmux attach -r` is read-only and keeps the debugging window. What stdin is
currently load-bearing on: LOOPFLOW.md advertises attach to *"answer an
interactive skill."*

The flowloop already solved this and execs did not inherit it —
`run_pass(content, answers, inbox_rx)` (`wave.rs:439`) threads answers from the
inbox. So closing the door is either porting that path to execs, or declaring
dispatched execs non-interactive by construction. They already run `lf -b`,
headless, so the second may be nearly true today and merely undocumented.
**Check this before writing the rule down** — it decides whether "no direct
control" is a doctrine change or a feature.

### 6. Backlogs are allowed

Agents may file tasks without running them. No enforcement that a task be solved
to be added.

This resolves the standing contradiction — `flowloop/README.md:35` (*"No
backlog… intent that isn't running yet lives in GOAL.md, memory, and chat — not
in a tracker"*) versus `LOOPFLOW.md` (*"Concrete tasks live in Linear"*) — in
Linear's favor. The flowloop README becomes wrong, not merely in tension, and
must be rewritten.

Be deliberate about what "no backlog" was buying. It made *"the open runs ARE
the wave's open tasks"* literally true, so `lf runs` and the `<in_flight>` fold
were a complete picture of intent. A task now has three states, not two: filed,
running, merged. The wave must **read** its backlog to select — `wave_pursue.md`
already reads live Linear tasks, so the plumbing exists. What "no backlog"
prevented was a tracker filling with intent nobody will do, and nothing else in
this design prevents it.

### 7. Mac surface

Each of these needs an MVP in Loopflow Mac, alongside Goals and Projects. The
wave is the only mind, so the wave is the only screen — everything below is a
pane on it, not a place to navigate to.

- **KRs** — every project's KRs for this wave, with their proof state. The bets,
  and whether they hold.
- **Open PRs** — the record of done-and-pending. Under no-backlog these *were*
  half the task model; they still are the closure evidence.
- **Active sessions** — the hands. Which flowloops are running, foreground or
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

### 8. Doctrine

`wave_pursue.md`'s frontmatter reads *"Delegate wave work through project and
task flowloops."* Its escape clause — *"Run execs directly only for hot, now
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

## Demo

```bash
lf wave infrastructure          # a wave with three projects, one chat
lf chat "how's release stability?"
```

The wave answers from its own memory. It has inhabited `technical-architecture`
— the project whose next move needed the thread — and drives it with
`lf loop task "..."`, blocking, in a forked worktree, to a merged PR.
`developer-efficiency` and `release-stability` run backgrounded, reporting up.
`lf runs` shows hands; nothing there is a conversation.

Steer without leaving the thread:

```bash
lf chat "actually skip the migration, just gate it"
```

If a pass is live, the message lands inside its running session at the next
tool boundary — seconds, not a pass boundary. If not, it journals and the pass
it wakes is born already caught up, because every mind receives recent chat at
birth.

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
- **A separate chat LLM vs the flowloop** was a false pair: the chat is an
  attachment mode on the running pass. One head; the thread outlives its
  bodies.
- **Does a loop child re-read the wave live?** Yes, verified: no `env_clear`
  in the pass-spawn path, and `FlowloopRun` worktrees are wave-named
  (`<repo>.<wave>.<run-id>`), so ambient resolution lands on the wave with or
  without `LFD_WAVE_ID`. The ear is at pass granularity.
- **The finish-wake hole**: done is a report on `<self>/report`; the flowloop
  subscribes to reports, not narration. A hand that finishes silently never
  reported — legible as its failure.

## Open questions

**Is inhabitation a flow-swap or just a blocking loop?** §1 says the wave's own
pass becomes the project flow (its reasoning in its own transcript, hands do
code). The demo blocks on `lf loop task` — a child process with a *private*
transcript, which breaks the trichotomy's cost table ("inhabit spends my
context budget" is false for a subprocess). Either inhabit = flow-swap and the
wave thinks *as* the project, or inhabit dissolves into "block on one loop" and
§1 mostly dies. The doc currently sells both.

**Who owns a backgrounded loop?** Shell `&` from inside a pass makes the loop a
grandchild of a process that dies in ≤30min. Server-owned detach
(`lf loop task --detach`, residency supervises, parks it in a named session)
fits everything else and gives read-only tmux something to attach to. But then
the `&` in this doc's own examples is wrong as written.

**Do project loops exist?** A delegated project's bit is "all KRs true" —
weeks, against `max_passes: 8`, held by a hand with no memory. Weeks-long bets
that accumulate learnings are mind-shaped. The clean alternative: tasks
delegate; projects inhabit or promote; only waves reside. That collapses the
verb table onto the noun table — task↔delegate, project↔inhabit-or-promote,
wave↔reside — and rewrites the demo, which currently backgrounds two projects.

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
