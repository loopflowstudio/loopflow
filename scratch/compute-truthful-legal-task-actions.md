# Compute truthful legal Task actions from full lifecycle state

W2-252 · wave `infrastructure` · project `developer-efficiency`

## Problem

Today the action a surface advertises for a Task Session is derived from
**status alone**, ignoring PR state, CI, review gates, and worktree/stack
evidence. Two concrete failures follow:

1. **`lf task status` carries no action at all.** `TaskSessionSnapshot`
   (`rust/loopflow/src/ops/task.rs:107`) is a durable-state dump — no
   `next_move`, no controls, no recommended action. Its text renderer
   (`rust/loopflow/src/bin/lf.rs:588`) prints phase/body/PR and stops. An
   operator reading it must *guess* what to do next.
2. **The action model that does exist lives only on `lf status`/`lf roadmap`
   and contradicts the PR/CI evidence.** `TaskAttentionControl`
   (`rust/loopflow/src/lf/commands/waves.rs:271`) is `{Start, Attach, Resume,
   Interrupt}` derived purely from `runtime.status` + `process.alive`
   (`derive_task_attention` lines 1509-1521). `active_pr_phase` is passed in
   but used only for the *reason string*, never for the *controls*. So a Task
   `waiting` on an open PR with **passing CI** gets `next_owner = Review`
   (correctly, via `next_move_for_task:1682`) yet `controls = [Resume]` — it
   advertises **Resume**, not **Review**. The fixture pins this wrong behavior
   (`tests/fixtures/dto/task_attention_states.json:50`, `dead_authored_commits`).
3. **The controls→button derivation is in Swift, not Rust**
   (`swift/LoopflowMac/Views/RoadmapView.swift:21`). The server and the Mac app
   can disagree by construction.

Who benefits: operators reading `lf task status` to decide what to do next, and
the Mac surface whose action buttons must match the server's notion of what is
legal. Why now: the developer-efficiency KRs demand that "needs me" vs "fine" be
instant and that routine actions feel like single actions — a status dump that
omits the legal next move, or a button that says Resume when the real move is
Review, breaks both.

## Implementation status

The Rust core is **already implemented**. A prior session landed the types and
derivation; what remains is the test/fixture/Swift migration and the matrix
test.

**Done (Rust):**
- `task/actions.rs` — `TaskAction` (6), `TaskActionStatus`, `TaskActionModel`,
  `TaskActionEvidence`, `ReviewGateState`, `derive_task_actions` (full truth
  table: review gate > PR phase > body liveness > status, predecessor overlay),
  `ci_failure_reason`, `one_action` helper.
- `TaskAttentionSnapshot.actions: TaskActionModel` replaces `.controls`;
  `TaskAttentionControl` enum deleted from type definitions.
- `TaskSessionSnapshot.actions: TaskActionModel` added.
- `derive_task_attention` takes `action_evidence: Option<&TaskActionEvidence>`,
  delegates to `derive_task_actions`.
- `task_snapshot` builds `TaskActionEvidence` (CI via `pr.fresh_ci()`, review
  gate from store, predecessor from parent PR) and calls `derive_task_actions`.
- `print_task_session` prints `action:` / `blocked:` lines.
- `BodyControl::Attach` added; `observe()` emits it for live bodies
  (`child_session.rs:792,806`).
- `review_gate_from` maps `interaction_reviews` → `ReviewGateState`.

**Remaining:**
1. **Rust tests (9 compile errors):** `projected_attention` test helper has the
   old signature (passes `Option<PrPhase>` as 4th arg; `derive_task_attention`
   now expects `Option<&TaskActionEvidence>`).
   `shared_attention_projection_covers_the_desktop_decision_table` asserts on
   `.controls` / `TaskAttentionControl::*` which no longer exist.
2. **4 fixtures:** `attention.controls` → `attention.actions` in
   `task_attention_states.json` (8 states; flip `dead_authored_commits` to
   `review`), `wave_detail.json`, `roadmap_snapshot.json`,
   `active_sessions_census.json`. Note: `observation.controls` (BodyControl)
   stay unchanged — only `attention.controls` (old TaskAttentionControl) become
   `attention.actions`.
3. **Swift mirror:** `WaveWorkMap.swift` — add `TaskAction`/`TaskActionStatus`/
   `TaskActionModel`, replace `TaskAttentionControl` + `controls` with
   `actions`, add `attach` to `BodyControl`. `RoadmapView.swift` —
   `roadmapTaskAction` reads `actions.recommended` (+ `runtime == nil` →
   `.run`); `roadmapTaskCanInterrupt` reads `observation.controls`.
4. **Swift tests:** `DTOFixtureTests`, `WorkAttentionTests`, `RoadmapViewTests`,
   `WaveLensTests` updated for the new types.
5. **Matrix test:** every `TaskSessionStatus` (8) × `PrPhase` (6 incl. none) ×
   predecessor (none/parent-open/parent-merged/parent-abandoned) ×
   `ReviewGateState` (none/requested/approved/changes-requested), asserting
   coherence + directive-named transitions.

All six open questions in `scratch/questions.md` are **validated by the existing
code** — the implementation confirms each assumption.

## The demo

A Task is `waiting`, its PR is `open`, required checks **pass**. Run:

```
lf task status W2-138
```

Text mode now ends with:

```
  action: review  (checks passed; awaiting review)
    blocked: resume  (awaiting review; resume after review to address feedback)
    blocked: complete  (PR is open, not merged)
```

`lf task status --json` carries the same `actions` object that `lf status` and
`lf roadmap --json` already serve the Mac app — one DTO, three surfaces, no
client-side re-derivation.

## Approach

Introduce one **legal-action model** computed by one **pure function** from a
total evidence bundle, and make every surface (`lf task status` text + JSON,
`lf status`, `lf roadmap`, the Mac app) consume it.

### The action enum — exactly the six the directive names

```rust
// rust/loopflow/src/lf/commands/waves.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskAction {
    Recover,      // restart/adopt a dead body for a non-terminal Task with no open PR in the way
    Resume,       // continue a parked session: retry publication, address review changes, unstick
    Review,       // a human must act: review the open PR, or answer an interaction-review gate
    StartNextPr,  // rotate to the next serial PR after a Review-disposition merge (next_slug)
    Complete,     // close the Task: gate-approved, or after a CompleteTask-disposition merge
    NoAction,     // nothing to do now: CI running, or terminal & settled, or body working live
}
```

### The model DTO — total, coherent, explains every block

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActionStatus {
    pub action: TaskAction,
    pub available: bool,
    /// Why the action is legal when available; the blocking fact when not.
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActionModel {
    /// The single best next action; always one of the available actions,
    /// or None only when no session exists. Never contradicts `actions`.
    pub recommended: Option<TaskAction>,
    /// All six actions in canonical enum order, each Legal or Blocked.
    /// Total coverage is what makes "mutually coherent" testable per state.
    pub actions: Vec<TaskActionStatus>,
}
```

"Mutually coherent" is enforced structurally: `recommended ∈ {available
actions}`, and the six entries are exhaustive and stable-ordered, so every
combination asserts to exactly one available subset with one recommendation.

### The evidence bundle + pure derivation

```rust
pub enum ReviewGateState { Requested, Active, Approved, ChangesRequested }

pub struct TaskActionEvidence<'a> {
    pub status: TaskSessionStatus,
    pub active_pr_phase: Option<PrPhase>,
    pub active_pr_after_merge: Option<AfterMerge>,
    pub active_pr_next_slug: Option<&'a str>,
    pub ci: Option<&'a CiObservation>,        // fresh CI reading for the open PR head
    pub process_alive: Option<bool>,          // Some(true)=live, Some(false)=dead, None=not-expected
    pub predecessor_phase: Option<PrPhase>,   // None=rooted on default branch; Some=parent PR phase
    pub review_gate: Option<ReviewGateState>, // None=no active interaction review
    pub abandon_intent: bool,
    pub local_progress_unsettled: Option<bool>,
}

pub fn derive_task_actions(evidence: &TaskActionEvidence) -> TaskActionModel { ... }
```

One pure function. `snapshot_task_detail` (status/roadmap path) and
`task_snapshot` (`lf task status` path) both build `TaskActionEvidence` from
their already-gathered data and call it. No network on the `lf task status`
path: CI comes from `pr.fresh_ci()` (stored reading), review gate and
predecessor from the store.

### Derivation rules (the truth table the directive pins)

Precedence: **review gate > active PR phase > body liveness > status**.

- **Review gate `Requested`/`Active`** → `Review` recommended ("awaiting review
  disposition"); `Resume`/`Complete`/`StartNextPr`/`Recover` blocked.
- **Review gate `ChangesRequested`** → `Resume` recommended ("address requested
  changes"); `Review` blocked ("review returned changes; resume to fix").
- **Review gate `Approved`** → falls through to PR-phase rules below, with the
  gate no longer blocking completion.

Then by active PR phase (gate not active):

- **Open + CI Pending** → `NoAction` ("required checks still running"); `Review`
  blocked ("checks still running"); `Resume` blocked ("awaiting CI").
- **Open + CI Failing** → `Resume` ("fix failing required checks: …"); `Review`
  blocked ("required checks failed"); `Complete` blocked ("PR open, not merged").
- **Open + CI Passing or no fresh reading** → `Review` ("checks passed; awaiting
  review" / "awaiting review"); `Resume` blocked ("awaiting review; resume after
  review to address feedback"). **This is the directive's named fix.**
- **Publishing** → `Resume` ("retry publication"); `Review` blocked ("PR not yet
  open on GitHub").
- **Working / no PR (body implementing):**
  - process alive → `NoAction` ("Task body is working"); live controls
    (attach/interrupt/steer) are the operational surface via `observation`.
  - process dead → `Recover` ("Task body stopped; recover to continue"); `Resume`
    blocked ("no parked step to resume — recover the body first").
- **Merged + `AfterMerge::CompleteTask`** → `Complete` ("PR merged; complete the
  Task"); `StartNextPr` blocked ("PR dispositions the Task complete").
- **Merged + `AfterMerge::Review`** → `Review` ("merged; answer the post-merge
  review"); after gate approved → `StartNextPr` if `next_slug` set, else `Complete`.
- **Abandoned PR (Task non-terminal)** → `Resume` ("PR abandoned; resume to
  re-publish or move on").
- **Terminal `Completed`/`Abandoned`** → `NoAction` ("Task is terminal"); all
  five lifecycle actions blocked. (`abandon_intent` set on a non-terminal Task →
  `NoAction` ("Task is being abandoned"), all others blocked.)

**Predecessor (stack) overlay:** when the active PR has `parent_pr_id` whose
parent is not `Merged`, `Complete` and `StartNextPr` are blocked with "stacked
on {parent}, which has not merged; land the parent first" — the same fact
`stacked_collapse` enforces (`ops/task.rs:288`). `Review`/`Resume`/`Recover`
stay legal (the child PR can still be reviewed/fixed). Parent `Abandoned` →
`Resume` ("parent PR was abandoned; re-base or abandon this stack").

### `Recover` vs `Resume` — the key distinction

- **Recover**: body is **dead** (process expected, not alive) **and no open PR**
  is sitting in front of the work. Restart/adopt a fresh body to continue
  unfinished implementation. Matches `supervisor_restart_bar` permitting restart
  in `Working` phase (`task/mod.rs:620`).
- **Resume**: the session is **parked with a named next step** — retry
  publication, fix CI/review changes, unstick a wait/block. Matches
  `supervisor_restart_bar` barring restart when a PR is `Open`/`Publishing`
  (`task/mod.rs:605-619`) — those are resume-to-act, not restart.

### Live body controls move to `BodyControl`

`Attach` and `Interrupt` are **operational controls on a live body**, not
lifecycle transitions, so they don't belong in the six. `Interrupt` already
lives on `BodyControl` (`child_session.rs:642`). Add `Attach` to `BodyControl`
and have `observe()` include it whenever the body is live (`Working`/`Stalled`).
The Mac surface reads `runtime.observation.controls` for attach/interrupt/steer;
the six-action model is purely lifecycle.

### Unstarted tasks (no session)

The six actions describe an **existing** session's lifecycle. A Task with
`runtime == None` gets `recommended: None`, all six `available: false` ("no Task
Session; start one with `lf task run`"). The Mac "Start" button stays
client-derived from `runtime == nil && !pm_completed` (existing fields) — launch
is out of the directive's six-action scope.

### Wiring

- `TaskAttentionSnapshot.controls: Vec<TaskAttentionControl>` → **replaced** by
  `actions: TaskActionModel` (`waves.rs:329`). `TaskAttentionControl` enum
  deleted (no back-compat — internal DTO, AGENTS.md).
- `derive_task_attention` (`waves.rs:1453`) builds `TaskActionEvidence` and
  delegates to `derive_task_actions`; it keeps producing `level`/`reason`/
  `process`/`local_progress` unchanged.
- `TaskSessionSnapshot` (`ops/task.rs:107`) gains `actions: TaskActionModel`,
  computed in `task_snapshot` (`ops/task.rs:2414`) from the same
  `derive_task_actions` after gathering CI (`pr.fresh_ci()`), review gate (store
  read of `interaction_reviews`), and predecessor (parent PR phase via
  `parent_pr_id`). `TaskSessionSnapshot` stays `Serialize`-only — the **shared
  cross-language DTO is `TaskActionModel`**, which lives on the already-mirrored
  `TaskAttentionSnapshot`.
- `print_task_session` (`lf.rs:588`) prints the `action:` / `blocked:` lines
  from `snapshot.actions`.
- Swift (`WaveWorkMap.swift`): add `TaskAction`/`TaskActionStatus`/
  `TaskActionModel`; replace `TaskAttentionControl` + `controls` with `actions`
  on `TaskAttentionSnapshot`; add `Attach` to `BodyControl`.
  `RoadmapView.swift:21` `roadmapTaskAction` reads `actions.recommended` (+
  `runtime == nil` → `.run`); `roadmapTaskCanInterrupt` reads
  `runtime?.observation.controls.contains(.interrupt)`.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does `lf task status` have the evidence to compute actions offline? | `task_snapshot` reads PRs + process liveness already (`ops/task.rs:2435-2439`). CI via `pr.fresh_ci()` is a stored-reading freshness check, no GitHub call (`task/mod.rs:427`). Review gate + predecessor are store reads. | No network added to `lf task status`. Confirmed safe. |
| Is the "passing PR advertises Resume" bug real and fixture-pinned? | Yes. `task_attention_states.json:50` `dead_authored_commits`: status `waiting`, `active_pr_phase:"open"`, `next_move.owner:"review"`, `controls:["resume"]`. `derive_task_attention:1521` catch-all gives `Resume` for any non-terminal non-active status. | The fix flips this fixture to `review` and tightens `derive_task_attention` to consult `next_move.owner`/PR phase for controls. |
| Where does `Attach` go when `controls` is removed? | `Attach` exists only on `TaskAttentionControl`; `Interrupt` is on both `TaskAttentionControl` and `BodyControl`. | Add `Attach` to `BodyControl`; both live controls read from `observation.controls`. Bounded ripple. |
| Does `PrSnapshot` expose the stack parent? | No — `PrSnapshot` (`waves.rs:367`) drops `parent_pr_id`; only the durable `TaskPr` carries it (`task/mod.rs:382`), and `print_task_session` reads it directly (`lf.rs:633`). | Predecessor evidence is built in the *builders* (which hold `TaskPr`), not from `PrSnapshot`. No need to add `parent_pr_id` to the wire `PrSnapshot` for this work. |
| Can `TaskSessionSnapshot` become a full cross-language DTO? | It's `Serialize`-only, no Swift mirror, no fixture today. Fully mirroring it is large and the Mac app doesn't decode it. | Don't. Share `TaskActionModel` (on `TaskAttentionSnapshot`, already mirrored) instead. `TaskSessionSnapshot` borrows it for output. Honors "same DTO" = the action DTO. |
| Is `next_move_for_task` now redundant? | Its `owner`/`reason` still drive `attention.level`/`reason` and roadmap section bucketing (`waves.rs:756`). The action model is a refinement, not a replacement. | Keep `next_move`; `derive_task_actions` consumes the same CI/PR inputs. |
| Does the review gate have a queryable state? | `interaction_reviews` table: `status in (requested,active,completed)`, `disposition in (approved,changes_requested)` (`0.11.015_interaction_reviews.sql:15,27`). | `ReviewGateState` maps directly. Store read in `task_snapshot`. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Add `Review` to `TaskAttentionControl` and patch `derive_task_attention` to consult PR phase | Minimal diff, but keeps a status+process-shaped enum and can't express `start-next-PR`/`complete`/`no-action`/blocked reasons. | Fails "recover, resume, review, start-next-PR, complete, and no-action are mutually coherent" and "illegal actions explain the blocking fact." Half-measure. |
| Make `lf task status --json` emit `TaskDetailSnapshot` | Truest "one shape," but loses `TaskSessionSnapshot`'s durable fields (lifecycle, gate_proposal, pm_writeback, directive versions) that `TaskDetailSnapshot` doesn't carry. | Replacing drops information operators need; the directive asks for the *action DTO* to be shared, not the whole snapshot. |
| Keep `controls` and add `actions` alongside | Two overlapping action vocabularies invite drift — the exact split-brain AGENTS.md DTO rules exist to prevent. | Don't maintain two models. Replace. |
| Derive `Attach`/`Interrupt` from a new `live: TaskLiveControls` field | Avoids touching `BodyControl`. | `Attach` is semantically a body control; `BodyControl` already holds `Interrupt`. A parallel field duplicates the concept. |

## Key decisions

1. **Six actions, total coverage.** Every `TaskActionModel` carries all six in
   canonical order, each Legal or Blocked with a reason. This is what makes
   "mutually coherent" + "illegal actions explain the blocking fact" testable
   per combination, not just assertable.
2. **`recommended` is the text-surface verb; `actions` is the proof.** Text mode
   prints `recommended` + the blocked reasons for actions an operator might
   wrongly reach for. JSON carries both.
3. **Recover ≠ Resume.** Recover = dead body, no PR in the way. Resume = parked
   session with a named next step. The split mirrors `supervisor_restart_bar`'s
   restart-vs-bar distinction, made positive.
4. **Live controls live on `BodyControl`; lifecycle actions are the six.** Adding
   `Attach` to `BodyControl` keeps the six pure and puts attach/interrupt where
   the body model already reasons.
5. **Share `TaskActionModel`, not `TaskSessionSnapshot`.** The action DTO is the
   shared cross-language wire type; `TaskSessionSnapshot` borrows it for output
   without becoming a full Swift-mirrored DTO.
6. **No backwards compatibility.** `TaskAttentionControl` and `controls` are
   deleted; fixtures and Swift mirror updated in lockstep (AGENTS.md: one
   implementation, no `v2_` shims).

## Scope

- In scope:
  - `TaskAction`/`TaskActionStatus`/`TaskActionModel` + `derive_task_actions`
    pure function (`waves.rs`).
  - Replace `TaskAttentionSnapshot.controls` with `.actions`; delete
    `TaskAttentionControl`.
  - Add `Attach` to `BodyControl`; `observe()` includes it for live bodies.
  - `task_snapshot` gathers CI + review gate + predecessor, computes `actions`;
    `TaskSessionSnapshot.actions` added; `print_task_session` prints them.
  - Swift mirror: new types, `controls`→`actions`, `BodyControl.Attach`,
    `roadmapTaskAction` reads `recommended`.
  - Fixtures: `task_attention_states.json` (8 states: `controls`→`actions`,
    flip `dead_authored_commits` to `review`), `roadmap_snapshot.json`,
    `wave_detail.json`. Rust + Swift fixture tests updated.
  - Matrix test: every Task status × PR state × predecessor × review-gate
    combination asserts coherence + pins directive-named transitions.
- Out of scope:
  - Mirroring `TaskSessionSnapshot` in Swift / making it `Deserialize` (the Mac
    app doesn't decode `lf task status`; `TaskActionModel` is the shared DTO).
  - Adding `parent_pr_id` to the wire `PrSnapshot` (predecessor is built in the
    builders from `TaskPr`; not needed for the action model).
  - Changing `next_move`/`attention.level`/roadmap section derivation (the action
    model refines, not replaces).
  - The `lf task run` launch affordance for unstarted tasks (client-derived, not
    one of the six).

## Done when

- `cargo test -p loopflow --lib waves` passes, including a matrix test over
  every `TaskSessionStatus` (8) × `PrPhase` (6, incl. none) × predecessor (none /
  parent-open / parent-merged / parent-abandoned) × `ReviewGateState` (none /
  requested / approved / changes-requested), asserting for each: `recommended ∈
  available`, no contradiction, and the directive-named transitions pinned
  (waiting + open + passing → `Review` legal & recommended, `Resume` blocked with
  the review fact; dead body + working phase → `Recover`; merged + CompleteTask →
  `Complete`; merged + Review + next_slug + gate approved → `StartNextPr`).
- `lf task status <issue>` text mode prints `action:` + `blocked:` lines; `--json`
  carries `actions` identical in shape to `lf status`/`lf roadmap --json`.
- `tests/fixtures/dto/task_attention_states.json` round-trips in
  `rust/loopflow/src/lf/commands/waves.rs` fixture tests and in
  `swift/LoopflowTests/{DTOFixtureTests,WorkAttentionTests,RoadmapViewTests,
  WaveLensTests}.swift`; `dead_authored_commits` advertises `review`.
- `cargo fmt` clean; `cargo clippy -- -D warnings` clean.
- `lf task status W2-138` on a real waiting-on-passing-PR session prints
  `action: review` (the demo).

## Wave alignment

Wave `infrastructure` serves the developer-efficiency project whose KRs include
"avoidable human-in-the-loop setup or repair steps found in agent runs fall to
zero" and "'needs me' vs 'fine' instant to distinguish." A status surface that
hides the legal next move — or advertises the wrong one — is an avoidable repair
step and a failure of the needs-me/fine split. This design makes the legal next
action a computed, explained, shared fact rather than a guess.
