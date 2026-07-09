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
| **Delegate** | mine | other | a private transcript — steerability, and every learning that stayed inside it |
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

### What a worker is

A worker has a voice and no room. `lf chat` from a dispatched worker lands in
the wave's journal through the family head's pen, attributed. Its dotted
channel `repo.wave.<run-id>` is a **byline, not an inbox**.

Its transcript is private: *"Trust worker summaries; do not reread worker
transcripts."* Its posts are public. What forks under delegation is neither
memory nor voice — it is the transcript.

Its ear is live at birth and deaf after, unless it opts into `lf sub` in a
background terminal. Conversation needs both halves. **A hand can report; only
a mind can converse.**

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

### The invariant

- Worker → wave: the transcript is private, the posts are public.
- Child wave → parent wave: the thread is private, `--parent` is public.
- Memory crosses freely downward, but only because `lf memory add` already made
  it an authored statement.

> **Nothing crosses a boundary unless someone wrote it down on purpose.**

Raw records stay home; authored statements travel. This is what lets
log-as-truth survive nesting: every log is complete for its own scope, and what
leaves it is what a mind decided to say.

`driver.rs:206` is the one exception — cap exhaustion fires `lf chat --parent`
with nobody deciding to. That is defensible; a wave that dies silently is worse
than a noisy one. A second exception would mean raw records have started
leaking upward.

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

### 2. PR work always dispatches

The wave home never lands. Therefore `lf pr land`'s `scratch/*` sweep never runs
in the outwave, and `scratch/loop.yaml` is safe there as the inhabited loop's
bit. Two commitments, one protecting the other.

`LFD_CHANNEL` (`wave_context.rs:79`) already wins over worktree-derived
identity, so a dispatched exec can hold its own worktree and still speak under
whichever byline is correct. Worktree and channel are already independent.

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

### 6. Doctrine

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
— the project whose next move needed the thread — and its passes are opening PRs
through dispatched execs in worktrees, which post progress into this one thread
under their own bylines. `developer-efficiency` and `release-stability` run as
delegated loops, reporting up. `tmux ls` shows work; nothing there is a
conversation.

```bash
lf project promote release-stability
lf chat --wave release-stability "drop the flaky retry, delete the test"
```

A second room, because you asked for one. The parent stops overhearing.

## Open questions

- Does `lf task`'s chat channel survive as a byline, or is it deleted in favor
  of event attribution? The dotted namespace currently conflates minds
  (`repo.wave`) with hands (`repo.wave.<run-id>`). `chat.rs` treats them
  uniformly; `--parent` walks a mind tree. Splitting them is where this design
  cashes out in code.
- Two levels is probably the honest depth limit — a grandchild's news reaches the
  root only if its parent re-authors it, and nothing structural says so.
- `wave_pursue.md` says "delegate all but one," which is still procedural, just
  at arity N−1. Two projects contending on the same files should not both run,
  and under no-backlog the real parallelism unit is in-flight runs, not
  projects. *Parallelism degree equals project count* is a legible invariant and
  legibility may beat optimality for something a human steers — but it is a
  choice, not a consequence.
- `flowloop/README.md:35` says *"No backlog… intent that isn't running yet lives
  in GOAL.md, memory, and chat — not in a tracker."* `LOOPFLOW.md` says
  *"Concrete tasks live in Linear."* These landed one commit apart (`5b363142`,
  then `601881cd`) and disagree. This design picks flowloop's side: if a wave
  becomes a task, the task is a running pass and Linear mirrors runs rather than
  sourcing intent. Decide it on purpose.
