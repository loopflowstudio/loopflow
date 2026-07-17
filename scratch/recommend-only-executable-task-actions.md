# Recommend only executable Task actions

## Problem

`lf task status` invents its own reasons. Four evaluators answer "what can this
Task do next", and they disagree:

| # | Evaluator | Where | Reads |
|---|-----------|-------|-------|
| 1 | `derive_task_actions` | `task/actions.rs:121` | evidence bundle: **active** PR only, review at **current phase coordinates** |
| 2 | `task_completion_gate` | `ops/task.rs:3868` | **all required** reviews + directive incorporation + active-PR settlement |
| 3 | resume preconditions | `ops/task.rs:4643` (`task_recovery_adoption`) + `task/runner.rs:1043` | worktree/branch, then **active PR — after the body has spawned** |
| 4 | liveness | `ops/task.rs:4089` | tmux, but only when `status.is_process_active()` |

Nothing forces them to agree, and they don't.

**Mechanism, W2-285 (PR #1037 merged, worktree present).** A merged PR has
`phase() == Merged`, and `PrPhase::is_active()` is `Working | Publishing | Open`
(`task/mod.rs:393`). So `prs.iter().find(|pr| pr.is_active())` returns `None` and
`active_pr_phase` is `None` (`ops/task.rs:4109`, `4131`). `derive_task_actions`
falls to `body_model`, whose `process_alive: None` arm recommends **Resume**
("resume the parked session") and blocks Complete with the hardcoded string
**"implementation not finished"** (`actions.rs:372`). That string is a literal,
not a predicate — it cannot be true or false, so it is printed three lines below
`PR 1: merged`.

The supervisor then follows the recommendation. `resume_task_async` checks
worktree adoption and PR reconcile, neither of which mentions an active PR, so
the command is accepted and generation 3 starts. Only inside the runner, after
the body exists, does `launch` demand an active PR and `bail!` — events
8773-8775, `"Task Session ... has no active PR"`. A recommended action, accepted
by the runner, spawning a body whose only possible outcome is that error.

**Why the review gate vanished.** Evaluator 1 reads
`interaction_review_at(session, phase_epoch, phase_iteration, phase_cursor)` —
the review *at the current step*. W2-285's real blocker, `ir_d191bfc6…`, sits at
an older coordinate, so the lookup returns `None` and the gate disappears from
status entirely. Evaluator 2 reads `required_reviews_for_task`, which is
phase-independent, finds it, and refuses correctly. Same Task, same instant, two
answers. (`review_gate_from` is itself copy-pasted into `ops/task.rs:4070` and
`waves.rs:1544` — the duplication is literal.)

**W2-286 (PR #1032 merged, worktree self-pruned)** takes the identical path to
the identical false string, while its true blocker is directive v2 unincorporated
(cur=2, inc=1). Worktree present in one, gone in the other, same output — the
worktree is not the cause. **W2-284 is the control**: its PR merged and it
completed from the same phase, because its body was alive and completed itself.
The gates are correct. Only the projection lies.

**Recover, same class.** `process_alive` is `Some(alive)` only when
`status.is_process_active()` — `Starting | Running` (`task/mod.rs:218`). A
generation with a recorded `Failed` outcome under a `Failed` session yields
`None`, and `body_model`'s `None` arm prints "body is not dead; the session is
parked" about a body the store records as dead.

**The cost.** Two supervisors escalated healthy waits as unrecoverable wedges,
filed a Task on a false premise, and a Wave still waits for a "terminal-restart
failure" trigger that cannot fire — retrying produces an honest refusal, not a
crash. A machine reading status cannot distinguish a healthy wait from a wedge,
so it escalates. That is Developer Efficiency's first KR ("avoidable
human-in-the-loop repair steps fall to zero") failing through a text field, and
its second ("no Task strands on a dead body") failing through a recommendation
that spawns doomed generations.

## The demo

```
$ lf task status W2-285
  PR 1: #1037 merged
  next: review — required human review ir_d191bfc6… (project) is completed without approval
  complete    blocked: Task W2-285 cannot complete until its gates close: required
              project review ir_d191bfc6… is completed without approval
  resume      blocked: Task W2-285 has no active PR to resume; PR #1037 merged

$ lf task resume W2-285
error: Task W2-285 has no active PR to resume; PR #1037 merged
```

The refusal is byte-identical to the status line, it arrives in milliseconds, and
**no generation is created**. Today the same command starts a body and kills it.

## Approach

Delete the parallel reason-generation. One predicate per action; every surface
renders the *same string value*, not a same-looking string.

### 1. The completion gate moves into the action model as typed blockers

`CompletionGate { satisfied: bool, blockers: Vec<String> }` (`ops/task.rs:3787`)
becomes typed and lives in `task/actions.rs`:

```rust
pub enum CompletionBlocker {
    Review { id: String, kind: ReviewerKind, state: ReviewBlockerState },
    Directive { version: u32 },
    UnsettledPr { which: String, phase: PrPhase },
}

pub struct CompletionEvidence { pub blockers: Vec<CompletionBlocker> }

impl CompletionEvidence {
    pub fn satisfied(&self) -> bool { self.blockers.is_empty() }
    /// The one sentence both `lf task status` and `lf task complete` print.
    pub fn refusal(&self, identifier: &str) -> Option<String>;
}
```

`ops/task.rs` builds the blockers from the store (impure, unchanged predicates:
`required_reviews_for_task`, `has_pending_directive`, `active_task_pr`) and hands
the value to the pure model. `task_complete` refuses with
`evidence.refusal(id).unwrap()`. `derive_task_actions` blocks `Complete` with
`evidence.refusal(id).unwrap()`. Byte-identity is structural — there is one
`format!` — not a convention two call sites are asked to honour.

`satisfied` stops being a stored field: it was a cached `blockers.is_empty()`,
i.e. a second place for the same fact to be wrong.

### 2. `Complete` availability *is* the gate. Lifecycle phase never decides it

`body_model` loses "implementation not finished" entirely. `Complete` is blocked
iff `CompletionEvidence` has blockers, with the gate's own words. When the gate is
satisfied but the Task has no merged PR, the honest reason is
`"no merged PR to complete from"` — a fact, checkable, and never printed for
W2-285/W2-286.

### 3. Evidence carries the settled PR, not just the active one

`active_pr_phase: Option<PrPhase>` conflates "no PR exists" with "the PR merged
and is therefore no longer active". Split it:

```rust
pub active_pr_phase: Option<PrPhase>,       // unchanged: Working|Publishing|Open
pub latest_settled_pr: Option<SettledPr>,   // NEW: the newest Merged|Abandoned PR
```

`derive_task_actions` routes a merged latest PR to `merged_pr_model` even with no
active PR — which is exactly W2-285's and W2-286's shape and exactly what
`merged_completing_pr` (`ops/task.rs:3914`) already does for the advance path. The
model stops reasoning from lifecycle phase and starts reasoning from durable PR
state, which is the directive's finding.

### 4. `Resume` requires an active PR — checked before a body exists

The runner's precondition (`runner.rs:1043`) becomes a shared function:

```rust
/// The runner's own start precondition, evaluated without starting anything.
pub fn resume_refusal(ev: &TaskActionEvidence) -> Option<String>
```

- `derive_task_actions` blocks `Resume` with it.
- `resume_task_async` returns it as an error, beside `task_recovery_adoption`,
  **before** `resume_session` mints a generation.
- `runner.rs:1043` keeps its `bail!` as a defensive invariant, now unreachable.

W2-285's doomed generation 3 cannot be created: the predicate that killed it is
the one that now refuses the command.

### 5. Recover liveness reads the recorded outcome

```rust
process_alive = match session.latest_process {
    None => None,
    Some(p) if matches!(p.outcome, Some(Failed{..} | Lost{..} | LegacyStopped{..})) => Some(false),
    Some(_) if session.status.is_process_active() => Some(tmux_session_exists(..)),
    Some(_) => None,
}
```

`Completed | Superseded | Interrupted` are settlements, not deaths, and stay
`None` — a body that finished its turn is not a body to recover.

### 6. `status --json` carries the review's identity

The refusal knows `ir_d191bfc6…`; status does not, while still reporting
`action: review`. A supervisor cannot tell *which* review, or whether a Human or
a Project owes the disposition — the difference between wait and act. So:

```rust
pub struct ActiveReview {
    pub id: String,
    pub reviewer_kind: ReviewerKind,
    pub requesting_generation: u32,
}
```

on `TaskActionModel`, populated whenever the model reports `review`.
`TaskSessionSnapshot` has no Swift mirror and no `tests/fixtures/dto/` entry, so
this is a Rust-only serde addition — no DTO drift surface (CLAUDE.md's wire-type
rule: the field is required-or-`Option`, no `#[serde(default)]`).

### 7. Delete the duplicate `review_gate_from`

Two byte-identical copies (`ops/task.rs:4070`, `waves.rs:1544`). One, in
`task/actions.rs`, beside the model that consumes it.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Is the merged PR really invisible to the action model? | Yes. `PrPhase::is_active()` = `Working\|Publishing\|Open` (`task/mod.rs:393`); `task_snapshot` selects `find(\|pr\| pr.is_active())` (`ops/task.rs:4109`) → `active_pr_phase: None` → `body_model` → `"implementation not finished"` (`actions.rs:372`). | Confirms the cause is evidence selection, not the model's logic. Fix #3 is the whole routing repair. |
| Why did W2-285's review gate not show in status? | Two different review queries. Model: `interaction_review_at(epoch, iteration, cursor)` — current step only. Gate: `required_reviews_for_task` → `list_interaction_reviews(wave)` filtered to policy `Require` — phase-independent. `ir_d191…` is at an older coordinate. | The gate's blockers must reach the model (#1). Keeping *both* queries: phase-scoped drives "answer this review now" precedence; the required set drives Complete. Collapsing to required-only would regress `Defer`-policy in-flight gates. |
| Would refusing resume up-front break the recovery path? | No. `resume_task_async` already refuses on worktree/branch/dirty preconditions before `resume_session` (`ops/task.rs:4643-4648`). The active-PR check joins that block. `task_recovery_adoption` itself reads `active_task_pr` (line 3102) only to compare branches, and no-ops when there is none. | #4 is additive at an existing refusal point. No new failure mode. |
| Does `lf pr next` need an active PR? | No — `ensure_working_pr_with_authority` rotates *from a settled PR* (`ops/task.rs:3531`, "Task has no settled PR to rotate from"). | `StartNextPr` stays available after a merge; only `Resume` gets the new bar. The two must not share a predicate. |
| Is a recorded `outcome` sufficient to call a body dead? | No. `ChildBodyOutcome` (`child_session.rs:224`) has six variants; `Completed`, `Superseded`, `Interrupted` are settlements. Only `Failed`, `Lost`, `LegacyStopped` mean died-without-settling. | #5 matches on the failure variants only. A blanket `outcome.is_some() → dead` would recommend Recover for every healthy parked Task. |
| Does the DTO change ripple to Swift/fixtures? | No. `grep -rln TaskSessionSnapshot --include=*.swift` → nothing; `tests/fixtures/dto/` has no task-snapshot fixture. | #6 is a Rust-only serde addition. No mirror to keep in lockstep. |
| Is this already fixed in an unmerged PR? (wave memory: this Project has twice nearly rebuilt open work) | Checked all five open PRs — #1040, #1036, #1035, #1034, #1018. Only #1034 touches `ops/task.rs`, and it scopes *review authority by session id*; nothing touches `task/actions.rs`, `task_completion_gate`, or `resume_task_async`. | Proceed. Rebase awareness for #1034's `ops/task.rs` hunks only. |
| Will `assert_coherent`'s exhaustive matrix cover the new evidence? | `every_status_pr_predecessor_and_gate_combination_is_coherent` already sweeps 8×6×4×4×4×3×3 = 27,648 cases and asserts exactly-one-available + non-empty reasons. | Extend its axes with `latest_settled_pr` and blocker presence. The invariant "recommended is available" is what stops a fix from re-introducing an unexecutable recommendation. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Fix the string: `body_model` prints "PR merged; awaiting gate" instead of "implementation not finished" | One-line diff | Treats the symptom. The recommendation would still be Resume, and resume would still spawn a doomed body. The directive's correction exists precisely because v1 assumed this was reason-text-only. |
| Make `derive_task_actions` async and query the store itself | One evaluator by construction | Destroys the pure-function property the model exists for, and every surface (`lf status`, `lf roadmap`, Mac app) would need a store handle to render a row. The evidence-bundle boundary is right; it was underfed. |
| Let `resume` on a merged Task rotate to the next PR automatically | The supervisor's intent "keep going" gets served | Silently converts a resume into a `pr next`. Rotation is a durable mutation; wave memory records what happens when rotation fires without authoritative reconciliation (ENG-20, W2-280 split-brain). Refuse and name the command. |
| Make `complete` bypass the gate when the PR merged | The two parked Tasks unpark immediately | Explicitly forbidden, and correctly: W2-285's gate names a real unapproved review. The gates are the only part of this system that was telling the truth. |
| Add `Acknowledge` as a seventh action for W2-286's directive blocker | The next move becomes a literal command | A new action is a new state to keep coherent across six surfaces, and `lf task acknowledge` is already reachable. The blocker's own sentence names it. Reject: the model has six actions because six is the closed set of lifecycle moves. |

## Key decisions

**The gate is the predicate; the model is a projection of it.** Not "the model
agrees with the gate" — the model *renders the gate's value*. Two strings can
drift; one value cannot. This is why `CompletionBlocker` is typed rather than
`Vec<String>`: the model needs to pick an owner from a blocker, and a prose
string cannot be asked which owner it names.

**`satisfied` is deleted, not kept in sync.** It was `blockers.is_empty()`
cached into a struct field — the same class of defect as the one being fixed,
one scope smaller.

**Refusal strings name the Task identifier, not the session id.** The runner's
current `"Task Session ts_e9ea4d… has no active PR"` is addressed to nobody: a
supervisor holds `W2-285`. One string, and it is the one a reader can act on.

**Both review queries survive.** Tempting to collapse to the required-review set
and call it deduplication. It would silently drop `Defer`-policy in-flight gates
from the Review recommendation. They answer different questions: "what must be
answered now" vs "what bars completion". Wave memory's design-review lesson —
verify, then prune — cuts the other way here: this one earns its place.

**No auto-merge** (`lf pr publish`, never `lf pr land`). Wave memory: auto-merge
answers only to CI and sails past any review disposition — the exact failure this
Task's evidence set is made of.

## Scope

**In scope**
- `task/actions.rs`: typed `CompletionBlocker`/`CompletionEvidence` + `refusal()`; `latest_settled_pr` evidence; `resume_refusal`; `ActiveReview` on the model; delete `"implementation not finished"`; own `review_gate_from`.
- `ops/task.rs`: build blockers from the store; `task_complete` renders the shared refusal; `resume_task_async` refuses before spawning; `task_snapshot` liveness from recorded outcome; populate `ActiveReview`.
- `lf/commands/waves.rs`: two evidence builders fed the new fields; delete the duplicate `review_gate_from`.
- `task/runner.rs`: keep the `bail!` as a defensive invariant.
- Tests: below.

**Out of scope**
- Weakening either gate. Both are correct.
- Reopening/rotating PRs, or any durable PR mutation (W2-286/#1032's territory).
- The `lf task status` human table layout beyond the reason strings it prints.
- Swift/Mac surfaces — no mirror exists for this snapshot.

## Done when

1. **One predicate, one string.** For every barred action, the status reason and
   the command's refusal are the same `String` value:
   ```
   cargo test -p loopflow --lib task::actions
   ```
   A test asserts, per barred action, `model.status(a).reason ==` the error text
   `task_complete` / `resume_task_async` actually produces.

2. **`"implementation not finished"` is unreachable for a merged PR.**
   `grep -rn "implementation not finished" rust/` returns only the test that
   asserts a merged Task never prints it.

3. **No doomed generation.** `lf task resume` on a merged Task with no active PR
   errors before any `ChildProcessGeneration` row is written. Test asserts the
   generation count is unchanged across the refused command — the actual W2-285
   failure (events 8773-8775), not a proxy for it.

4. **The next move names the real gate and owner.**
   - W2-285 fixture (merged PR, required review completed-without-approval, worktree present) → recommends `review`, reason names `ir_…` and its reviewer kind; `complete` blocked with the gate's sentence.
   - W2-286 fixture (merged PR, directive cur=2/inc=1, worktree pruned) → `complete` blocked naming directive v2; `resume` blocked naming the absent active PR.
   - W2-284 fixture (merged PR, gate satisfied, body alive) → completes, unchanged. **The control must stay green**; a fix that parks W2-284 has broken the thing that works.

5. **Recover agrees with the record.** A generation with `Failed` outcome →
   `recover` available; `Completed`/`Superseded` → not.

6. **`status --json` carries review identity.**
   `lf task status W2-285 --json | jq '.actions.active_review.id'` → `ir_…`.

7. `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test -p loopflow` to
   completion (wave memory: a failing lib target makes cargo skip later targets;
   green-looking is not green).

### Sabotage (the test that proves the tests)

Two strings generated by code I just wrote match trivially. So, per the
directive and wave memory's sabotage rule, each guard must be shown to go red:

| Sabotage | Test that must fail |
|----------|--------------------|
| `CompletionEvidence::refusal` returns a second, different `format!` for the model | status/command agreement, per action |
| `latest_settled_pr` forced to `None` | W2-285 + W2-286 fixtures (recommendation reverts to Resume) |
| `resume_refusal` returns `None` always | no-doomed-generation test |
| `process_alive` reverts to the `is_process_active()` gate | recover-liveness test |

The wave-memory precedent is exact: a value-asserting test can pass against the
defect it names when the fixture supplies the real value (W2-280's
`unwrap_or_default`). The agreement test is the one most at risk of passing for
free — it is asserting `x == x` unless the two renderers are genuinely separate
call sites reading one value. If a sabotage does not go red, the test is pinning
the fixture and gets rewritten, not accepted.

## Measure

- **Before:** `lf task status W2-285` recommends `resume`; `lf task resume W2-285`
  is accepted and creates a generation that dies with
  `"Task Session ts_e9ea4d… has no active PR"`. Doomed generations across
  W2-285: 1 (generation 3, events 8773-8775).
- **After:** recommendation is `review` naming `ir_d191bfc6…`; `resume` is refused
  in-process; doomed generations: 0.
- **KR line:** Developer Efficiency KR 2 — "zero Sessions sit in failed awaiting a
  manual resume, and zero durable commands are left orphaned against a dead
  generation". A recommendation that can only produce a dead generation is that
  KR's generator. KR 1 — "avoidable human-in-the-loop repair steps fall to zero":
  the escalations this defect caused were the repair steps.
