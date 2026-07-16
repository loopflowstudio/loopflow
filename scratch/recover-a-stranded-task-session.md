# Recover a stranded Task Session

## Problem

When a Task body dies, recovery is **partial and silent**, not absent. Some dead
bodies come back on their own; others sit at `failed` for hours with a durable
status message instructing a *human* to type `lf task resume` — an instruction no
machine ever executes.

A sweep of the developer-efficiency Project on 2026-07-16 found 13 Sessions
stranded at `failed` with frozen generations. One manual resume revived 13/13,
including a gen-9 Session. Nothing was defective. Nothing was asking.

The cost compounds: a stranded Session silently misses every fix that ships after
it froze. W2-210 was pinned to an ephemeral binary that temp-reaping deleted;
that defect was already fixed and merged (W2-225), and a fresh resume re-resolved
it correctly. The Session had simply frozen at a pre-fix failure mode and stayed
dead.

Beneficiary: every Task Session, and the humans who currently hand-sweep them.

## The demo

```
$ tmux kill-session -t lf-task-W2-267-46981e68     # kill a live body outright
$ sleep 10 && lf task status W2-267
W2-267  running  gen 5   recovered automatically 6s ago (attempt 1/3)
```

No human typed `lf task resume`. Beside it, a `completed` Task whose body was
reaped as `lost` shows no recovery event at all — recovery never touched it.

## Findings that reshaped this design

The parent Project Session's directive was corrected mid-flight, and the
correction was itself half-right. Both readings were verified against the live
registry (`~/.lf/loopflow.db`, 117 Task Sessions) rather than reasoned about.

### The `settled` gate is real, but it is not the whole defect

`project_session/runner.rs:810-835` gates `relaunch_inactive_process` behind
`settled`. A Task with an open, healthy PR whose body dies falls to the `else if`
where `wake_warranted` is false (no required check failed), so nothing relaunches
it. That path is real.

But the dominant strand is a *different* line. `ops/task.rs:1806` is the **sole
writer** in the codebase of:

```rust
let reason = "task process is missing; resume the same Task Session with `lf task resume`";
record_task_failure(store, session, reason, reason.to_string()).await
```

This is the strand, verbatim: a durable field containing a human instruction, and
no reader.

### The 13 had open PRs *and* W2-129's bar is real — both, and that is the key

The parent's evidence said the 13 had open PRs, so recovery should fire
"regardless of PR settlement." But `supervisor_restart_bar` (`task/mod.rs:689`)
deliberately bars restart under an `Open` PR, and its doc comment records why: the
2026-07-14 W2-129 failure, where a wake launched generation 2 under open PR #878
and began re-doing delivered work. "Relaunch regardless of settlement" would
re-introduce W2-129.

The live registry resolves the contradiction. The two surviving `failed` Sessions
are the two archetypes:

| Task | `status_reason` | `outcome.kind` | PR phase (derived) |
|---|---|---|---|
| W2-135 gen 9 | `task process is missing; resume the same Task Session…` | `lost` | OPEN |
| W2-212 gen 1 | `codex_error: You've hit your usage limit` | `failed` | OPEN |

W2-135 shows `failed` under an **OPEN** PR — which line 1806 cannot produce, because
line 1765 routes an Open PR to `Waiting` instead. The only consistent history: the
body died while the PR was **Publishing** (publication requested, GitHub receipt
not yet observed), line 1806 marked it `failed`, and `reconcile_task_pr` *later*
observed the receipt and moved the phase to Open. It has looked like an open-PR
strand ever since.

So both readings were right about their evidence and wrong about the cause. PR
phase is a **lagging, mutable** signal — it changed after the strand formed. It is
the wrong thing to gate recovery on, in either direction.

### The trigger gap is worse than "only while an iteration is live"

Two loops exist, and neither reliably observes a dead body:

- The 5s tick (`supervise_project_task_bodies`, `ops/task.rs:1816`) filters to
  `process.state == Active && live_sessions.contains(&process.tmux_name)`
  (`:1832-1839`) — a body whose tmux is **gone is excluded by the filter**. The
  stall supervisor structurally cannot see a dead body.
- `reconcile_process_liveness` — the only path that *does* handle a dead body — is
  called from `inspect_outcome`, whose sole caller is
  `project_session/runner.rs:331`, inside **flow-turn-boundary** handling.

So a dead Task body is noticed only when its parent Project happens to finish a
flow turn. A Project that is waiting, blocked, or asleep never notices. This is
why Sessions sat frozen for hours.

## Approach

**One dispatcher, keyed on durable status, bounded by a durable attempt count.**

The rule, in one line: *recovery fires when the Session's own durable status
claims a body that does not exist.*

This single predicate subsumes the entire PR-phase argument:

- A cleanly delivered Task parks at `Waiting`. Recovery **never** fires on
  `Waiting`, so W2-129 is preserved by construction — not by a PR bar.
- A Task at `Running`/`Starting` with no live body believes it is working and is
  not. That is unambiguously a strand, whatever its PR phase. This covers the 13.
- PR settlement stops gating recovery (the parent's correction holds) *without*
  re-introducing W2-129 (my objection holds). Status does the gating; phase does
  not.

Three pieces:

**1. Classify (pure).** `plan_stranded_recovery(&TaskSession, attempts) ->
StrandedPlan`, in `child_session.rs` beside the existing `plan_body_recovery`.
Clock-free, store-free, unit-testable.

```rust
pub(crate) enum StrandedPlan {
    /// Body alive, Task terminal/abandoning, or Waiting/Blocked on purpose.
    LeaveAlone,
    /// The body vanished without recording a terminal outcome. Start gen+1.
    Redispatch { attempt: u32 },
    /// The body recorded why it stopped, or recovery is spent. Say so once.
    Surface { reason: String },
}
```

**2. Discriminate on `outcome.kind`, structurally — not by parsing prose.**

- `ChildBodyOutcome::Lost` = *vanished without recording a terminal outcome*.
  Nobody chose this; it is Loopflow's own reap verdict. **Recoverable** (13/13).
- `ChildBodyOutcome::Failed { reason }` = the body **recorded** why it stopped.
  Something already decided this is terminal. **Never blind-retry**; surface once
  with the reason and the handoff command.

This is what "read `status_reason` before retry" should mean. Regexing
`status_reason` for `codex_error:` or `usage limit` is a string-matching trap that
rots the first time a provider rewords an error. The structural signal is already
in the outcome tag and needs no parsing. W2-212 (`failed`, codex usage limit) is
barred by its tag, not by its wording.

Crucially, `Lost` is **not** read as "failure" — the triage's central trap. `Lost`
+ `completed` (W2-171/#913, W2-226/#965, W2-227, W2-233/#982) is a body reaped
after success. The status check rejects it before the tag is ever consulted, so
nothing keyed on `lost` alone can chase completed work.

**3. Dispatch through the existing path.** `relaunch_inactive_process`, reached via
`ChildSession::launch(store, LaunchIntent::Recovery)`. No new launcher, no new
process, no daemon.

### Where it runs

Extend the **existing 5s tick** (`supervise_project_task_bodies`) to a second
cohort: Sessions whose durable status claims a body with no live tmux. The tick
already runs every 5s in the Project runner's `select!` loop **independent of flow
turns**, so recovery stops depending on the parent finishing an iteration. This
honours the stated invariant at `ops/task.rs:1810-1815` — *"deliberately
parent-driven: Project and Task Sessions do not grow a second watchdog process"* —
because it widens an existing control tick rather than adding a watcher.

### On "do not add a second re-dispatcher"

Agreed, and this **removes** one rather than adding one. Two mechanisms currently
decide to relaunch: `reconcile_process_liveness`'s pending-Resume bridge
(`ops/task.rs:1775-1780`) and `inspect_outcome`'s `settled` gate
(`runner.rs:812-820`). Both become special cases of the one classifier. The
`settled` gate is a redundant, stricter, ad-hoc second policy that *shadows*
`supervisor_restart_bar`; deleting it and letting the bar plus the status
predicate decide is a net subtraction.

Two dispatchers could not double-launch even if they raced: `reserve_task_process`
(`store/sqlite/child_sessions.rs:477-530`) CASes on
`COALESCE(process_generation,0) = <prev>` **and** a `finished` lease. A second
launcher loses the race and gets `Ok(None)`. Correctness here rests on that
fencing CAS, not on there being exactly one caller.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Do the 13 have open PRs, as the directive says? | Yes — but W2-135 reached `failed` via a line unreachable when the phase is Open (`task.rs:1765` routes Open → `Waiting`). It crashed while **Publishing**; the receipt was observed later. | PR phase is a lagging, mutable signal. Gate on status, not phase. Resolves the directive's contradiction with W2-129. |
| Would "relaunch regardless of settlement" re-introduce W2-129? | Yes, if keyed on phase. No, if keyed on status: delivered work parks at `Waiting`, which recovery never touches. | Status predicate replaces the PR bar for this intent, preserving W2-129 by construction. |
| Is `lost` a safe recovery trigger? | No, alone. `completed` + `lost` is a body reaped after success (4 confirmed). | Check status terminality **first**; consult the tag second. |
| Can `status_reason` classify terminal failures? | It is a free-form `String` (`task/mod.rs:631`) with no enum, written from ~6 sites. | Do not parse it. `outcome.kind` already carries the distinction structurally. |
| Does the 5s tick see dead bodies today? | No — it filters on `live_sessions.contains(&process.tmux_name)` (`task.rs:1832-1839`). | Widening this filter is the change; the tick itself is sound. |
| Is there a daemon/trigger to reuse? | No. The `triggers:` registry was deleted (`wave_config.rs:339`); `lf cron` is a launchd plist manager, wrong tier. The wave resident is the only self-reviving tier. | Reuse the parent runner's tick. Do not build a daemon. |
| Can two relaunchers race? | `reserve_task_process` CASes on generation + `finished` lease. | Safe by fencing, not by convention. |
| Does generation count bound retries? | No — W2-178 is at gen 17 from legitimate PR rotations. | Needs a distinct, progress-relative attempt counter. |
| Is "never touch a completed Task" already satisfied? | Partly. `inspect_outcome:787` skips terminal, and `task_recovery_adoption:793` skips unsafe worktrees — but the 5s tick has neither. | Verified as directed: reuse `terminal_or_abandon_bar` in the classifier; do not duplicate the adoption check. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Gate recovery on PR settlement (as first directed) | One-line change | Re-introduces W2-129; phase is lagging and mutable, and *changed after* the strand formed |
| New recovery daemon / `lf cron` sweeper | Survives a dead parent | Violates the stated no-second-watchdog invariant; `lf cron` is launchd plumbing at the wrong tier |
| Parse `status_reason` for terminal errors | Catches the codex case today | Free-form string from ~6 sites; rots on the first provider reword. The tag is already structural |
| Bound retries by generation number | No new state | Generations increment on legitimate PR rotation (gen 17); would strand healthy long-running Tasks |
| Relaunch from `reconcile_process_liveness` directly | Closest to the bug | It is called from a flow-turn boundary; fixing it there leaves the trigger gap intact |

## Key decisions

1. **Recovery keys on `status.is_process_active()` with no live body — never on
   `lost`, never on PR phase.** The one predicate that is neither lagging nor
   ambiguous. It is also the reason W2-129 stays fixed for free.
2. **`Lost` vs `Failed` is the retry discriminator, structurally.** Loopflow's own
   reap verdict (`Lost`) is recoverable; a body's recorded verdict (`Failed`) is
   not. No prose parsing.
3. **Bounded at 3 consecutive attempts without intervening progress**, counted from
   durable `task_events`, not from the generation number. On exhaustion, one
   `Surface` with the real reason — the classifier then returns `LeaveAlone`, so
   exhaustion is self-consistent and cannot mint dead generations.
4. **The ci-fix wake stays untouched and distinct** (W2-230/W2-229). Its trigger is
   a failing required check on a live open PR; recovery's trigger is a dead body.
   They share `relaunch_inactive_process` and nothing else.
5. **W2-249 still owns attempt resolution.** Recovery triggers; W2-249 settles.
6. **A dead *Project* Session still strands its Tasks. Out of scope, stated
   plainly.** Recovery runs on the parent's tick, so a parent that is not running
   observes nothing. Fixing that means the wave tier (the only self-reviving one)
   and is a separate bet. This design removes the *flow-turn* dependency, not the
   *live-parent* dependency — an honest partial win, not a silent one.

## Scope

**In scope**
- `plan_stranded_recovery` pure classifier + unit tests (`child_session.rs`)
- Widen `supervise_project_task_bodies` to the dead-body cohort (`ops/task.rs`)
- Replace `record_task_failure("…resume the same Task Session…")` at
  `ops/task.rs:1806` with a recovery verdict
- `LaunchIntent::Recovery` + `recovery_restart_bar` (`ops/child.rs`, `task/mod.rs`)
- Retire the `settled` gate at `runner.rs:812-820` in favour of the classifier
- `TaskEventKind::BodyRecoveryAttempted { generation, attempt, reason }` for the
  durable attempt count and observability
- Integration test: kill a body, assert `running` with no human action; assert a
  `completed` Task's reaped body triggers nothing

**Out of scope**
- Reaping/classifying orphaned processes (`b9843f04`) and subprocess reaping on
  body exit (`9e3c71ba`)
- Attempt resolution after recovery (W2-249 / `bd3d4f6d`)
- The ci-fix wake path (W2-230 / W2-229)
- Recovering Tasks under a **dead Project Session** (decision 6)
- Provider handoff *automation* — recovery surfaces the handoff command; it does
  not choose a provider

## Done when

```bash
cargo test -p loopflow stranded_recovery      # classifier truth table
cargo test -p loopflow recovery_integration   # kill-a-body → running; completed → untouched
```

Observable: `tmux kill-session -t lf-task-<id>` on a live Task returns it to
`running` within ~10s with no human action, and `lf task status` names the
recovery attempt. A `completed` Task whose body is reaped records no recovery
event. W2-212's archetype (`Failed` outcome) surfaces its reason and handoff
command **once** and does not mint a second generation.

## Measure

Baseline, from `~/.lf/loopflow.db` on 2026-07-16: **2** Sessions at `failed`, one
recoverable (W2-135, frozen at gen 9), one terminal (W2-212). The pre-sweep
baseline was 13 stranded across 57 tasks (~23%).

After: across one week of real runs, zero Sessions sit at `failed` awaiting a
manual resume with a recoverable (`lost`) outcome. Query:

```sql
select count(*) from task_sessions
where status='failed' and process_outcome_json->>'$.kind'='lost';
```

Target: 0. Terminal (`failed`-outcome) Sessions may legitimately be non-zero —
they are surfaced, not stranded — which is exactly the distinction this design
turns on.

## Wave alignment

- **Intent** (`GOAL.md`): "turns repeated friction and operational risk into
  system capability." A hand-swept 13-Session strand is precisely repeated
  friction; this deletes the manual sweep rather than documenting it — the GOAL's
  "Do not document avoidable manual work as a workflow; delete it with code."
- **KRs**: advances *"Avoidable human-in-the-loop setup or repair steps found in
  agent runs fall to zero"* (the literal `lf task resume` instruction in a durable
  field is that step) and *"No Task strands on a dead body: zero Sessions sit in
  failed awaiting a manual resume."*
- **Process**: this touches the worker/wave runtime, so per `GOAL.md` it gets this
  scratch design first, reviewed by the parent Project Session before implementing.
- **Memory**: MEMORY.md's *"Task body recovery is gated on settled"* is confirmed
  and now made precise — the gate is real, but the dominant strand is
  `ops/task.rs:1806`, and the `lost` caveat recorded there is load-bearing.
- **New risk introduced**: recovery still depends on a live parent runner
  (decision 6). Named, not hidden.

## Open question for the reviewer

Decision 6 is the one I would most like challenged. Recovery on the parent's tick
fixes the flow-turn dependency but not the dead-parent case. The alternative — a
wave-tier sweep — is the only self-reviving tier, but it crosses a tier boundary
and would be a second dispatcher, which the directive rightly forbids. I chose the
bounded win. If the dead-parent case is the one you actually care about, this
design is one increment short and should be re-scoped before implementation
rather than after.
