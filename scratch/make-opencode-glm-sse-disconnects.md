# Make OpenCode GLM SSE disconnects observable and recoverable

## Problem

During a real Product pursuit, several GLM-via-OpenCode bodies went **hollow**
after an SSE disconnect: reported healthy/complete while producing no usable
work, forcing a manual handoff to Claude Opus. Task loops that silently "succeed"
with an empty body break the wave's core trust contract (a dispatched loop must
land its PR *or* stop with an actionable record — never silently stall).

Root architectural gap: loopflow derives turn boundaries from opencode's
`session.status` and treats `idle` as proof of a completed turn
(`opencode_mapping::map_status` → `TurnCompleted { Completed }`). But a truncated
**upstream** GLM stream (GLM → opencode server) can surface to loopflow as
`session.status: idle` carrying zero content. Loopflow trusts the status, maps it
to `Completed`, and advances the flow step as if the skill ran. Meanwhile the
harness's own `/event` stream disconnect (opencode server → loopflow) *does* fail
the body today, but it leaves an orphaned open turn and always retries the **same
flaky provider**, with no path to a backup.

Two truths the design must fix:
1. **Observability** — a turn that emitted no usable work must never read as
   `Completed`, regardless of what status opencode reports.
2. **Recoverability** — a disconnect-class failure must, by default, route the
   next body to a configured backup provider (a different one — GLM flakiness is
   not cured by relaunching GLM), retaining the failed generation as evidence,
   and retry the same provider only when replay safety is proven.

## The demo

Run the deterministic fake opencode SSE server in "drop after `active`, before any
content" mode, drive one task body against it, and watch loopflow:

```
task INF-…> body failed: opencode_hollow_body: turn completed with no assistant output
task INF-…> body handed off: opencode:glm-5.2 → claude:opus (opencode disconnected before producing work)
```

The event ledger shows a `Failed` opencode generation (gen 1, reason recorded)
**and** a `BodyHandedOff` to the backup, with the next generation running
claude:opus against the same worktree, directive, and PR. No `Completed` turn, no
advanced flow step, one writer.

## Approach — two shared layers, no GLM name special-case

### Layer 1 — Observable hollow-body boundary (harness + mapping)

The single load-bearing insight: **detect hollowness by content, not by status.**
This is correct whether opencode reports an upstream truncation as `idle` or
`error`, so we never have to resolve opencode's exact reporting behavior to be safe.

`opencode_mapping::ReaderState` gains per-turn content tracking. A turn is
**substantive** if, between `TurnStarted` and its close, it emitted any of:
`TextDelta`, `ReasoningDelta`, a tool `ItemStarted`/`ItemCompleted`, or
`DiffUpdated`. On turn close:

- `session.status: idle` **and substantive** → `TurnCompleted { Completed }` (unchanged).
- `session.status: idle` **and non-substantive** → `TurnCompleted { Failed }` +
  `Error { code: "opencode_hollow_body" }`. **Never `Completed`.**
- `session.status: error` → `TurnCompleted { Failed }` (unchanged; already correct).

Harness `/event` disconnect (`opencode.rs` sse_task): when `response.chunk()`
returns `None`/`Err` **while a turn is active**, emit `TurnCompleted { Failed }`
for the orphaned turn *before* `Error { opencode_disconnected }`, so every
`TurnStarted` gets a terminal close and the journal never carries an open turn
past a disconnect. Reason distinguishes phase:
- no `TurnStarted` seen or seen with zero content → `pre-content disconnect`.
- content seen then cut → `mid-stream disconnect`.

Decode-gap guard: if a turn closes `idle` with `TurnUsage.output_tokens > 0` but
zero mapped content parts, that is a **harness decode failure**, not a hollow
model turn — fail with `opencode_decode_gap` so a mapping regression can never
masquerade as an empty-but-successful turn.

All three new failure codes join `is_terminal_harness_error` semantics as
appropriate (`opencode_hollow_body`/`opencode_decode_gap` are turn-terminal;
`opencode_disconnected` stays session-terminal).

### Layer 2 — Recovery: safe retry vs. backup handoff (child supervision)

A disconnect/hollow failure is **not replay-safe by default**: the truncated turn
may have completed an external side-effecting tool (a Command/File/commit) whose
replay would double-apply. Classify and route:

- **Replay-safe** iff the failed turn completed **no** external side-effecting
  tool item this turn (pre-content, reasoning-only, and text-only truncations are
  replay-safe; a completed Command/File item makes it unsafe).
- On a disconnect-class body failure
  (`opencode_disconnected` / `opencode_hollow_body` / `opencode_decode_gap`):
  1. **Backup configured and not yet exhausted** → hand the next generation to the
     backup agent via the existing `handoff_{task,project}_body` path, append
     `BodyHandedOff { from, to, reason }`, retain the failed opencode generation.
     This is the **default** for non-replay-safe failures and the directive's
     preferred path.
  2. **Else replay-safe** → allow the supervisor's existing same-agent respawn
     (one bounded retry — a transient blip on a body that did nothing durable).
  3. **Else** (no backup, not replay-safe) → stop `Failed`/`Blocked` with an
     actionable non-convergence record; **never** silently re-run a side-effecting
     body.

Reuse the proven handoff machinery — it already preserves Session identity,
worktree, directives, PR identity, provider history, and bumps the generation
fence (see `handoff_task_body` test at `task/runner.rs:2328`). The only new pieces
are the **automatic trigger** on disconnect-class failure and a **`backup_agent`
config knob**.

Config: `backup_agent: Option<String>` on the wave `GOAL.md` frontmatter, threaded
into `TaskSession`/`ProjectSession` lease state. Example: `backup_agent: claude:opus`.
Absent → no auto-handoff (fall to path 2/3). A `backup_used` marker on the session
prevents ping-pong (we route to backup at most once per body failure chain).

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does opencode report a truncated upstream GLM stream as `idle` or `error`? | Unknown and not reliably observable without live GLM flakiness. If `error`, we already fail correctly; if `idle`, we currently mint a hollow success. | Detect by **content**, not status — correct under both. This is why the fix does not depend on resolving opencode's behavior. |
| Can a legitimate turn produce zero content? | In the loopflow flow model every body runs a skill expected to produce edits, commands, or a substantive reply. A zero-output turn is never useful work. | Safe to treat any non-substantive `idle` close as hollow. Reasoning/text/tool/diff all count as substantive, so a "thinking then answering" turn is fine. |
| Is retrying the same body after a disconnect safe? | The truncated turn may have completed a side-effecting tool (commit, `pr publish`) whose replay double-applies; loopflow resumes bodies by re-running the skill, not replaying the transcript. | Gate same-agent retry on replay-safety (no completed Command/File item this turn); prefer a **fresh** backup body that re-reads current state over replaying. |
| Does a backup mechanism already exist? | Yes: `ChildBodyHandoffRequest { agent, provider, reason }` + `handoff_{task,project}_body` + `BodyHandedOff` events set the next generation's agent/provider and preserve identity. No automatic trigger; no provider-level failover today (existing "backup profiles" are auth accounts, not providers). | Add only the auto-trigger + `backup_agent` config; do not build a parallel failover system. |
| Where must one writer be guaranteed? | Generation fencing: only the current generation's lease may write. Handoff bumps the generation. | A late-arriving write from the dead opencode body is fenced out; the backup body owns the next generation. Prove with a fencing test. |
| Will closing the orphaned turn on disconnect double-fail the body? | The runner's `Error` handler already calls `finish_failed`; adding a preceding `TurnCompleted { Failed }` only closes the turn ledger — the body still fails once. | Emit the turn close for journal honesty; the single `finish_failed` still owns the body outcome. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Catch hollowness only in the runner, not the mapping | Keeps mapping dumb | Every consumer (task, project, wave) re-implements "was this turn substantive"; opencode semantics must live in one place — the mapping. |
| Provider-level auto-failover inside the harness | Fewer moving parts at the supervision layer | The harness owns exactly one provider process; provider choice is Session lease state. Routing belongs where generation/handoff already live. |
| Trust `idle` = Completed; let the flow's own work-check catch empties | No mapping change | The flow advances the step on `Completed` *before* any work check exists; the boundary must be at turn close, not after the step advances. |
| Status-based detection (special-case "idle with no usage") | Simpler | Fragile — depends on opencode always attaching usage, and on which status it picks for truncation. Content-based is invariant to both. |

## Key decisions

- **Content-based, not status-based** hollow detection — the one insight that makes
  the fix correct without pinning down opencode's truncation reporting.
- **Backup handoff is the default recovery; same-agent retry is the exception** —
  matches the directive and the reality that a flaky provider is not cured by
  relaunching it.
- **Reuse the existing handoff machinery** — add only the automatic trigger and one
  config knob, not a second recovery system.
- **Every `TurnStarted` gets a terminal `TurnCompleted`** — disconnect closes the
  orphaned turn so the ledger never lies about an in-flight body.

## Scope

- **In scope:** `opencode_mapping` hollow detection + per-turn content tracking;
  `opencode.rs` disconnect turn-closure; new `Error` codes
  (`opencode_hollow_body`, `opencode_decode_gap`); replay-safety classification;
  automatic backup handoff on disconnect-class failure in task + project
  supervision; `backup_agent` config knob; deterministic fake-SSE test server +
  the four-case matrix; audit/event coverage for the new reasons.
- **Out of scope:** Claude/Codex harness behavior (Claude fails the turn on crash
  by construction; Codex disconnect is its own provider surface); Product-specific
  prompts or any GLM-name special case; the live 10-body GLM validation run
  (operator step, tracked in Done-when); the affected-receipt replay writeup
  (evidence step, done during iterate against durable receipts).

## Slices (reviewable, in order)

1. **Observability (this slice):** `opencode_mapping` + `opencode.rs` hollow &
   disconnect turn-closure, new error codes, unit tests. Self-contained; makes
   every hollow/disconnect body *visible* with an actionable reason. No behavior
   change to recovery yet — a hollow body becomes a `Failed` body (already
   handled by every consumer's `finish_failed`).
2. **Fake-SSE harness test server + matrix:** deterministic server covering the
   four disconnect points, asserting exact body outcome and no false `Completed`.
3. **Recovery:** `backup_agent` config + automatic backup handoff on
   disconnect-class failure + replay-safety gate, in task + project supervision.
   Fencing + no-duplicate-side-effect tests.
4. **Evidence:** root-cause writeup of the affected receipts; operator 10-body run.

## Done when

- `cargo test -p loopflow` covers the fake-SSE matrix — disconnect (a) before
  headers/content, (b) after metadata before content, (c) mid-tool-call, (d) after
  a durable completion event — each asserting exact body outcome, **no false
  `Completed`**, single writer, no duplicate external side effect, and correct safe
  retry vs. backup handoff.
- A disconnect-class failure with `backup_agent` set produces a `BodyHandedOff` to
  the backup and retains the failed opencode generation.
- Root-cause writeup of the affected Product receipts (from durable evidence, not
  the provider name).
- **Operator:** ten real GLM/OpenCode Product bodies, zero hollow successes, every
  forced disconnect visible with reason + recovery owner.

## Measure

- Baseline: current behavior maps a content-free `idle` turn to `Completed`
  (reproduce with fake-SSE case (a)/(b) → assert the *old* code would advance the
  flow step; the new code fails the body).
- After: 0 hollow `Completed` turns across the fake-SSE matrix and the operator's
  10-body run; 100% of forced disconnects carry a reason code and a named recovery
  owner (retry vs. backup).

## Root-cause analysis (from durable evidence)

The hollow bodies were not a GLM bug, a prompt failure, or a wave supervision
gap. They were a **measurement gap** in the opencode harness mapping layer:
loopflow derived turn completion from `session.status: idle` without checking
whether the turn actually produced work. A truncated upstream stream (GLM →
opencode server) that surfaced as `idle` with zero content was indistinguishable
from a real completed turn.

### The mechanism

1. **Upstream truncation**: GLM's SSE stream to the opencode server drops
   mid-response. Opencode maps the truncated turn to `session.status: idle`
   (not `error`) — the turn is "done" from its perspective, just empty.

2. **Status-based completion (the bug)**: `opencode_mapping::map_status` treated
   `idle` as proof of completion: `Active → Idle` with no content check →
   `TurnCompleted { Completed }`. The runner advanced the flow step as if the
   skill ran.

3. **No recovery path**: even when the harness's own `/event` stream
   disconnected (opencode server → loopflow), the body failed with a generic
   `"provider event stream closed"` reason and the supervisor respawned the
   same flaky provider. No backup handoff, no replay-safety check.

### What the fix does (three layers)

**Layer 1 — Observability**: A turn is substantive only if it emitted text,
reasoning, a tool call, or a diff. `idle` with no substance → `Failed` +
`opencode_hollow_body` (never `Completed`). A disconnect while a turn is open
closes the orphaned turn `Failed` before the `Error`, so the journal never
carries an open turn past a disconnect. A `decode_gap` code distinguishes "the
model produced tokens we failed to map" from "the model produced nothing."

**Layer 2 — Recovery routing**: A disconnect-class failure
(`opencode_disconnected` / `opencode_hollow_body` / `opencode_decode_gap`) is
classified by `classify_disconnect_recovery`: if a `backup_agent` is configured
and not already in use, the body hands off to the backup via the existing
`handoff_{task,project}_body` path. If no backup and the turn was replay-safe
(no completed Command/File item), a same-agent retry is allowed. Otherwise the
body stops with a non-convergence record — never silently re-running a
side-effecting body.

**Layer 3 — Fencing**: The handoff finishes the dead process before switching
the agent, so a late-arriving write from the dead opencode body hits a
`LeaseRevoked` error — the process state is `Finished`, not `Active`, and the
lease check rejects it. The backup body owns the next generation.

### Why content-based detection is correct

Opencode's truncation reporting is not reliably observable without live GLM
flakiness: it may map a truncated upstream stream to `idle` or `error`. If
`error`, the old code already failed correctly. If `idle`, the old code minted
a hollow success. Detecting by **content** (did the turn emit anything usable?)
is correct under both — it does not depend on which status opencode chose.

## Implementation status

- **Slice 1 (Observability):** Done. `opencode_mapping` per-turn content
  tracking, hollow/decode-gap error codes, `close_orphaned_turn` on disconnect,
  `drain_turn_failure_reason` in both runners, `is_disconnect_class_failure`
  classifier, 14 mapping + 12 harness tests.
- **Slice 2 (Fake-SSE matrix):** Done. Deterministic fake SSE server covering
  all four disconnect points (pre-content, after-active, mid-tool,
  after-durable), each asserting no false `Completed`, every `TurnStarted`
  closed, `opencode_disconnected` error present.
- **Slice 3 (Recovery):** Done. `backup_agent` in `WaveConfig`, replay-safety
  tracking (Command/File `ItemCompleted`), `classify_disconnect_recovery`
  decision, `handle_body_failure` in both runners, `BodyHandedOff` + fencing
  test proving the old lease is rejected after handoff.
- **Slice 4 (Evidence):** This writeup. Operator 10-body run is an operator
  step (out of scope for code).
