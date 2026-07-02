# Looping waves as attachable sessions — v2 (post-review rewrite)

Rewritten from scratch after three adversarial reviews (engineering-simplicity via
Codex/gpt-5.5, product–infrastructure alignment, seed-investor focus). v1 proposed
new `WaveAgent`/`Dispatch` types, a `needs-input` session status, a ticker→supervisor
demotion, and an lfq cockpit — before a single user had watched one loop ship real
work. All three reviews independently said: too much, too early, and some of it
duplicates machinery that already exists. This version keeps the one real primitive,
reuses what's built, and normalizes the concept set so the model maps 1:1 to what a
Wave actually is.

Predecessors: `scratch/goal-authoring-and-dispatch.md`, `scratch/goals-launch-plan.md`.
Vision: `wave/goals/README.md`.

## What we're building

A Wave is an always-on looping agent: a session, configured by a goal, that reads its
roadmap, dispatches subagents to do the work, and steers them — including subagents
running flows with **interactive steps** a human can drop into and answer. That
capability is the point, not a deferrable extra. The design job is to make that model
real and coherent end-to-end, using primitives Loopflow already has rather than
minting new ones, and validating on a real repo as we go (dogfood on Cadenza).

The engineer and product-infrastructure reviews sharpened *how* to build it — fewer
new types, reuse what exists, fix the branch-collision and loop-closing gaps. Those
corrections are folded in below. What's rejected is the "don't build it yet" framing;
the looping-agent model with steerable subagents is the thing we're making.

## The one real primitive: dispatch-through-lfd

All three reviews agreed on exactly one load-bearing idea, so it leads:

**Every dispatched flow gets its own lfd-created `TerminalSession`.**

The wave's loop runs in a session. When it decides to do work, it does **not** shell
out `lf <flow>: <task>` inside its own pane — that collapses orchestrator and child
into one terminal and, for an interactive step, hijacks the orchestrator's stdin.
Instead it asks lfd to launch the flow, and lfd creates a separate tmux-backed
`TerminalSession` with its own stdin and attach handle. That launch path already
exists (`executor/wave/mod.rs:458-491`) and already launches interactive
(`build_lf_inline_command(..., true, ...)`). The work is to invoke it *on the agent's
decision* instead of *once per ticker slot*.

This is the whole design. The rest is what NOT to build around it.

## The object model — a Wave is a durable hub; agents are its incarnations

The reviews said "don't mint `WaveAgent`/`Dispatch` types." Going further: normalize
to a 1:1 concept map (CLAUDE.md's Clarity goal), which first means being honest that
**a Wave is more than a looping agent.**

**A Wave is a durable namespace, and its name is the join key.** The name unifies
these facets independent of whether any agent is running:

| Facet | Lives in | Durable without a live agent? |
|---|---|---|
| Intent | the goal it loops (`GOAL.md`, via a pointer) | yes |
| Work index | `wave/<name>` branches, PRs, `<repo>.<name>` worktrees | yes |
| Memory | `wave/<name>/MEMORY.md` — roadmap, decisions, learnings, curated | yes — the continuity substrate |
| Agent | its current canonical `TerminalSession` incarnation | the *agent* is durable — it **is** the Wave, via its memory; the incarnation is not |

The agent is not an ephemeral thing the Wave *has*. **The Wave *is* the durable
agent** ("a Wave is a goal agent"). A `TerminalSession` is an *incarnation* — the
live process. Kill the process, the supervisor relaunches: same durable agent, new
incarnation. There is one **canonical incarnation** (the persistent loop the
supervisor keeps alive) and possibly **ephemeral extras** (an ad-hoc `lf goal` you
watch, a human exploratory session, a fork) — all `TerminalSession`s on the wave,
told apart from the canonical one by whether the supervisor owns their liveness.

Three durable concepts, one escalation edge, zero new runtime types:

```
Wave              the durable named hub — intent + work-index + ledger + context + agent identity
  ├─ agent()      → its canonical live TerminalSession (wave_run_id: None, source: wave_agent)
  │                  (+ possibly ephemeral extra incarnations)
  └─ dispatches   → [WaveRun]  work units filed under the name (task, worktree, branch, pr, repair)
        └─ session → that work's live TerminalSession (wave_run_id: Some)

TerminalSession   a live attachable process; role read off (source, wave_run_id)
AttentionItem     the loop's human-escalation edge (kind, terminal_session_id, status)
```

"WaveAgent" and "Dispatch" are **roles, not rows** — read off `(source,
wave_run_id)`. `wave.agent()` is a *relationship* (the canonical live incarnation),
not an embedded field: storing a `TerminalSession` on `Wave` would put runtime state
in the durable record and go stale the moment the process dies.

What normalizing *deletes* (net-negative lines):
- `WaveRunSnapshot` sub-struct dissolves — `direction`/`area` cut, `flow` (the reflex
  tag) moves directly onto the run.
- Flow-walking fields (`step_index`, `execution_cursor`, `flow_parents`) — no referent
  once the agent dispatches instead of walking a step cursor (delete once the safety
  behavior they carry is re-homed).
- Runtime state migrates off `Wave` onto the incarnation — but the **durable facets
  stay**. Wave is a hub, not a pointer.

One semantic shift to name out loud: `WaveRun` stops meaning "an iteration" and starts
meaning "a dispatch." One agent iteration spawns 0..N dispatches, so run-count ≠
iteration-count. The honest name is `Dispatch`; keep the `WaveRun` *type* until a
rename is cheap — same rows, not a new concept.

**Open (not settled here):** whether `Wave.status`/`iteration` are cached on the hub
or derived from the incarnation + open attention items. Flagged under Open forks.

### Wave on disk: two files, both injected

`wave/<name>/` is exactly two files:

- **`GOAL.md`** — intent. Frontmatter (metrics, `roadmap` pointer, `agent`, `workers`,
  `primary_flow` bias) + body (the loop prompt). Renames the landed lowercase
  `GOAL.md`; the resolver moves with it.
- **`MEMORY.md`** — the wave's curated memory: roadmap progress, decisions, learnings,
  the context it has accumulated. Replaces the old N-numbered roadmap files; a
  repo-local roadmap folds in here, an Asana roadmap lives external with MEMORY.md as
  the working memory of it.

**Both are injected into every context the wave runs** — the agent reads them each
loop, and every dispatched subagent gets them in its prompt. Consequences:

- **This *is* the context-strategy facet** — no `area` successor, no context knob. The
  wave's context is its own two files.
- **The loop gains a write-side.** The operating loop is read `GOAL`+`MEMORY` →
  dispatch → **fold results back into `MEMORY.md`** → repeat. Memory is an output of
  the loop, not just an input — the concrete mechanism behind "work compounds."
- **Cross-dispatch learning with no database.** Dispatch N inherits what dispatch 1
  discovered because it was written to `MEMORY.md` and rides into every child's
  context. No shared store.
- **Self-limiting by construction.** Because `MEMORY.md` is injected everywhere, it
  can't sprawl without blowing every subagent's budget — the budget pressure forces
  curation. It is bounded and compressed (lean on `token-compress`/`summaries`), like
  a `MEMORY.md` index over curated notes, not an unbounded append log.
- **Continuity across incarnations.** A relaunched agent reads `GOAL.md` + `MEMORY.md`
  and resumes. This is the substrate that makes the canonical agent durable while its
  process is not.

## Interactive steps: this is Attention's fit moment (like lfq)

v1's `needs-input` `TerminalSessionStatus` was wrong twice over, and both the
engineer and product-infra reviews caught it independently:

1. It's a wire enum mirrored across Rust/Swift/Python/store — the exact drift the
   design's own DTO rule (and the metrics/primary_flow decision) exists to prevent.
2. **The semantic already has a home.** `lfd/types/attention.rs` has
   `AttentionKind::Interactive` (and `Algedonic`) carrying a `terminal_session_id`,
   with a `Surfaced → Viewed → Resolved` lifecycle and restart reconciliation
   (`reconcile_attention_items`, `attention.rs:103`). A parked interactive step
   *already* raises an attention item bound to its session; a completed session
   already resolves it (`completed_terminal_session_resolves_interactive_attention`).

But the reviews leaned on Attention as if it were bedrock, and it isn't. Attention/
Inbox is the **same class of API as lfq**: an obvious surface built ahead of a real
use (`routes/attention.rs` calls itself "the primary API for `lf` to register that
it needs human input"), with no strong daily pull, retrofitted since. So the move is
not "reuse the existing shape" — it's the same reframe we gave lfq: **the looping-
wave model is the demand that finally gives Attention its purpose.** Parked children
and algedonic blocks in an autonomous loop are the real, recurring reason a human
gets pulled in. Claim Attention as *the loop's human-escalation channel* and shape
it to that job.

Concretely: don't add `needs-input`; a parked Dispatch surfaces as an
`AttentionItem(Interactive)`. But treat Attention's lifecycle as **open to
redefinition** here — verify `Surfaced → Viewed → Resolved` + reconciliation
actually fit a parked-child-in-a-live-loop (does an unanswered item time out? does
the dispatching agent see its own child's open item? does resolving it require
attach, or can the agent auto-resolve?) rather than assuming the retrofitted shape
holds. lfq and Attention are the two speculative APIs this work pulls into their
true form — the *same* pattern, not a dependency and a feature.

## The autonomy contract (the contradiction v1 hid)

Product-infra #2 found the real tension: v1 justified all the attach plumbing with
"subagents run interactive steps — a human must jump in," but the goal metric is
"≥20 *unattended* iterations" and the operating prompt says "operate autonomously,
don't stall." Both cannot hold. Resolve it explicitly:

- **Headless dispatches auto-answer safe interactive steps.** A dispatched flow runs
  headless by default; its interactive steps take the executive-decision path
  (`Surface::Headless` already tells steps to decide and keep moving).
- **Only genuine ambiguity escalates** — raised as an `AttentionItem(Interactive)`,
  which is loop-visible (the agent can see a child is parked) and human-visible (via
  the attention queue). Escalation is the exception, not the loop's critical path.

This makes attach a *steering affordance*, not a *precondition for progress*. The
loop runs unattended; a human can drop into any child when they want to, and a truly
stuck child asks for help rather than hanging silently.

## Fan-out is negative throughput until branches are isolated

Product-infra #1 is the biggest technical gap v1 missed. Parallel runs today share
one remote branch and stack PRs on it (`executor/helpers.rs:210-269` gives per-run
worktrees + *temporary local* branches, but the run tracks the wave branch and all
runs push to the same remote; `wave/mod.rs:539` auto-PRs per run). That machinery was
built for one-run-per-iteration. The operating prompt now wants N *independent* tasks
in parallel — which needs N independent branches and PRs. As-is, `workers: 3` = three
agents racing to push one branch.

Therefore:

- **Start at `workers: 1`.** No fan-out until isolation exists — one dispatch at a
  time can't collide with itself.
- **Fan-out is gated on task-scoped branch+PR isolation** — each dispatch owns a
  branch derived from its task, lands independently. That isolation is a *precondition*
  for `workers > 1`, promoted out of "later polish." Until it lands, `workers` is
  capped at 1 in code, and we say so rather than pretend fan-out works.

## Close the loop: PR state feeds re-measure

Product-infra #5: the loop doesn't actually close. There's no land trigger and no
post-land re-measure (`wave/mod.rs:530-589` auto-creates a draft PR and advances the
branch; landing is the external merge queue). The agent "re-measures" by reading
repo/roadmap files — but dispatched work lives in an *open PR*, not in main, so the
agent can re-dispatch a roadmap item whose PR is still in flight.

Fix: feed each `WaveRun`'s `{status, pr-state}` into the loop's **read** step, so
"re-measure" sees in-flight work. The record already carries `{pr, status}`; wire it
into the loop's input, not just the cockpit view. Without this, momentum leaks in the
gap between "PR open" and "PR landed" and the agent duplicates work.

## One looper, chosen — not demoted-and-kept

Product-infra #4 / engineer #5: v1 kept the ticker (as a "supervisor" that still
polls) *and* added a resident session — two loopers, and it filed loop cadence as an
"open fork" though cadence is exactly what decides the cost. Decide it:

- **The loop is self-paced inside the session.** The agent runs an iteration, then
  sleeps/waits before the next. The external process shrinks to **crash-restart
  only** — is the session's `lf` process alive? If it died, relaunch. It does not
  schedule iterations.
- Do **not** delete the ticker's safety fields (`iteration`, `cycle_start_iteration`,
  max-iteration pause, repair lineage) until that safety behavior has a named new
  home. Demote the ticker's *role*; keep its guardrails until replaced.

## The 2am story (undesigned in v1, required here)

An always-on session fails in ways a cold ticker never did. Engineer #4 / product-infra
note: `reconcile_terminal_sessions` (`wave/mod.rs:330`) treats "tmux gone" as "run
completed with an exit code" — correct for a one-shot run, **wrong for a loop
session**, where tmux-gone means *relaunch*, and "tmux alive but `lf` dead" leaves a
zombie pane the reconciler ignores.

Required before any always-on claim:

- A **heartbeat**, not pane-existence, decides liveness. A loop session writes a
  liveness marker each iteration; the supervisor reads it.
- Reconciliation for a `source = "wave_agent"` session: tmux-gone → relaunch;
  heartbeat-stale + tmux-alive → zombie, kill+relaunch; healthy → leave.
- Every reconciliation decision logs `wave_id, session_id, tmux_name, argv,
  exit_code, reason`. Failure must not look like silence.

## lfq: two verbs, not a cockpit

Engineer #7 / CEO #5: reshaping lfq into a session-first cockpit is product scope, not
plumbing needed to prove the model. lfq has no session commands today
(`rg terminal_session python/loopflow/` is empty) but the endpoints exist
(`list_terminal_sessions_handler`, `attach_terminal_session_handler`, tested). Add
exactly:

- `lfq sessions [wave]` — list sessions (grouped by `wave_id`), flag ones with an
  open `AttentionItem(Interactive)`.
- `lfq attach <id>` — `POST attach` for `tmux_name`, then `tmux attach -t <name>`.

That's the whole "jump into a subagent and answer it" flow. `lfq logs` stays the
passive tail. Everything else waits.

## GOAL.md: one source of truth

Carried from the prior session and reaffirmed by engineer #6: authored handles live
in one place. `Wave` holds the **intent pointer** (`goal` — the resolution key), not
copies of the authored contract. `GOAL.md` (via `WaveConfig`) owns the prompt,
`metrics`, `roadmap`, `primary_flow` bias, and `agent` — read at launch/render time,
not mirrored as columns. This retires the just-landed `metrics` column too. This is
the *intent* facet only; the durable hub keeps its other facets (name-as-index,
memory) — it is not shrinking to a pointer. `MEMORY.md` is the sibling file (above).

(Cost, acknowledged: API/Concerto read authored fields from the worktree files, not a
DB row. Acceptable — intent and memory live in the repo by design.)

## Sequence

Ordered by what unblocks what, dogfooded on Cadenza throughout — not gated behind a
single demo clip.

**Foundation — the tighten unit + the launcher.** Ship the already-scoped unit
(`Surface::Ide`, `wave.goal` never-nil, `.lf/goals/` resolver). Build
`prepare_goal_launch` + the operating prompt, and `Target::Goal` discovery so
`lf <goalname>` launches a wave's agent as a watchable session. This is the
canonical-incarnation constructor.

**Keystone — dispatch-through-lfd.** The agent's dispatch verb asks lfd to launch each
flow as its own `TerminalSession` (promote `wave/mod.rs:458-491` from ticker-owned to
agent-owned). Each dispatch is attachable. `workers: 1`.

**Interactive escalation via Attention.** A parked interactive step in a dispatch
raises `AttentionItem(Interactive)`; the agent auto-answers safe ones, escalates only
ambiguity. Shape Attention's lifecycle to the parked-child case. Add `lfq sessions` +
`lfq attach`.

**Close the loop.** PR/run-state into the read step so re-measure sees in-flight work;
no double-dispatch.

**Durability.** Self-paced loop; supervisor = crash-restart of the canonical
incarnation via heartbeat + the reconciliation rules above.

**Earn fan-out.** Task-scoped branch+PR isolation per dispatch; only then lift
`workers > 1`. Budget primitive (deferred) becomes the real gate; until it lands,
`workers` *is* the budget and is a hard cap.

## Explicitly cut (and why)

- **New `WaveAgent`/`Dispatch` persisted types** — they're `TerminalSession` +
  `WaveRun` with different field values (engineer #1). Vocabulary, not schema.
- **`needs-input` `TerminalSessionStatus`** — the semantic belongs to Attention, not
  a mirrored session enum (engineer #3, product-infra #3). Attention itself is *not*
  cut but *claimed* — this work is its fit moment, same as lfq.
- **lfq cockpit reshape** — two verbs suffice (engineer #7, CEO #5).
- **Deleting ticker safety fields now** — keep guardrails until re-homed (engineer #5).
- **Two loopers** — one self-paced loop in the incarnation; the supervisor only
  crash-restarts, it does not schedule iterations (product-infra #4).
- **Fan-out before branch isolation** — negative throughput otherwise (product-infra #1).

## Open forks

- **`Wave.status`/`iteration` cached vs derived** — the last runtime state clinging to
  the durable hub. Deriving is purer; caching reads cheaper.
- **`MEMORY.md` curation policy** — how it's bounded and compressed (size budget, what
  gets kept vs summarized vs dropped, who prunes and when). Injection pressure forces
  *some* discipline; the explicit policy is unspecified.
- **Persistence backend** A1 (lfd drives vendor cloud) vs A2 (scaffold + hand off) vs
  tmux-local — decide when demand for unattended-overnight is real, not before.
- **Remote attach** — `tmux attach` is local; remote lfd needs ssh-tmux or Concerto's
  streamed route. Local-first now.
- **Machine-checked metrics** vs prose self-judgement — only where a hard exit is
  needed.
