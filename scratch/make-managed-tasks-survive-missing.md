# Make managed Tasks survive missing lifecycle flows

## Problem

Managed Tasks pin lifecycle flow names so their execution plan survives process
and provider restarts. That pin currently proves only that the stored string is
non-empty. When the builtin `task` flow was removed, existing Tasks retained it
as their loop flow: LOO-193, LOO-150, and LOO-152 now report `resume` as the
legal action, but `resume` reserves a Run whose body immediately fails with
`flow not found: task`. LOO-178 shows the adjacent failure: status itself cannot
explain recovery when the recorded worktree is absent.

The same missing-path assumption breaks brand-new Tasks. At
2026-07-21T18:31:02Z, `lf task run` published a Working Task PR before its
declared worktree existed. For roughly two minutes `lf task status`, `lf task
wait`, and the product roadmap failed even though Task creation was still in
progress and no Task body was expected yet. A planned worktree and a lost
worktree are different states; flattening both to “working” or “missing” makes
the shared read model lie.

The user needs Task identity to outlive catalog evolution. Repair must retain
the same Task row, worktree path, dirty files, provider transcript, lifecycle
position, and serial PR chain. It must not make a removed name appear valid
through an alias.

This advances the Loopflow API KR: “Task loops earn trust by streak: over one
week of real work, every dispatched task loop either lands its PR unattended or
stops with an actionable non-convergence record — zero silent stalls, zero
human rescues inside the window.” It also protects “One model everywhere,
continuously”: status and the lifecycle command share one preflight instead of
projecting different truths.

## The demo

Against an isolated copy of the recorded LOO-193 shape, `lf task status LOO-193
--json` says that `resume` will migrate the retired loop pin from `task` to
`slice`. `lf task resume LOO-193` updates only that pin, preserves the dirty
file and PR sequence, and starts the same Task through `slice`; repeating status
on an unknown missing flow or absent worktree recommends no executable action
and names the recovery required.
During a delayed new-Task launch, status, zero-timeout wait, and roadmap all
remain readable and say that the worktree is initializing; after five minutes
without progress the same durable marker becomes actionable incomplete-startup
recovery instead of waiting forever.

## Approach

Make lifecycle executability one preflight shared by status, explicit resume,
and every Run launch.

1. Resolve and expand all three pinned flows with the same Task phase rules used
   at creation. A plan is executable only when every phase still resolves and
   first/loop contain skills while finally contains skills followed by optional
   ops.
2. Recognize one data migration: an exact loop pin of `task` may become `slice`
   only when the otherwise unchanged candidate plan fully validates. Status
   reports this pending migration without writing it. Explicit `lf task resume`
   persists the repaired plan before PR reconciliation, Run reservation, or
   provider launch.
3. Reject every other invalid persisted reference. Status returns `no_action`
   with the same validation error that `resume` returns. Restoring the named
   repo-local flow or shipping a separately reviewed migration is the recovery;
   arbitrary missing names are never guessed from current Project defaults.
4. Preflight automatic launches without applying migrations. A supervisor
   cannot reserve a Run against a missing flow; a user-triggered resume is the
   explicit boundary that repairs the known legacy default.
5. Treat a missing worktree as inspectable but not executable. Status skips
   git/PR reconciliation that requires the directory and reports how to restore
   the exact recorded path and PR branch before resume. Resume refuses before
   changing lifecycle, PR, Run, provider, or Work state.
6. Keep creation on the existing resolver: all Project defaults and CLI
   overrides must resolve and expand before the Task row and first PR are
   persisted.
7. Publish `WorktreeInitializing` in the same SQLite transaction as the Task,
   Working PR, and initial Steer. Create the worktree next, then publish the
   existing `PrStarted` event as the ready boundary. Read surfaces treat the
   fresh marker as initialization, not missing local work; a marker older than
   five minutes becomes an explicit restore/resume refusal.
8. When resident supervision finds an invalid lifecycle, write one actionable
   `Failed` event and park. Later ticks compare the exact latest receipt and do
   not append another event, reserve a Run, rotate a PR, or launch a body. A
   changed or repaired lifecycle naturally changes the preflight result.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Is `task` still resolvable through another catalog path? | No. The builtin registry asserts `get_builtin_flow("task").is_none()`. `task-kickoff`, `task-gate`, `task-design`, `slice`, and `ship` remain available. | Repair the stored reference; do not add an alias. |
| Is `task` an explicit user choice or a historical default? | Migrations `0.11.013`, `0.11.014`, and `0.11.022` introduced/preserved `task` as the default loop flow. A read-only production projection found 200 exact pins. | The only automatic mapping is exact `task → slice`, the current replacement for that retired default. |
| Can creation already persist an unavailable flow? | `task_run` and `task_start` call `resolve_task_lifecycle` before creating Task/PR state; it loads, expands, and phase-validates every flow. | Preserve this boundary and add a proof that failed resolution leaves no Task. |
| Where does the bad Run begin? | Persisted plans are not revalidated until the runner loads the current flow after Run reservation. Status derives `resume` from liveness/PR state only. | Validate before every reservation and let action derivation consume the same result. |
| Can resume preserve dirty work? | `task_recovery_adoption` explicitly allows dirty files while an active PR owns the worktree and performs only reads before adoption. | Run lifecycle preflight/migration before existing reconcile/launch mutation; never stash, reset, rotate, or replace the worktree. |
| Does a merged predecessor change the repair? | LOO-150 and LOO-152 retain several merged PRs plus one active working successor. Lifecycle lives on the Task, not a PR. | Update only the Task plan; assert the complete ordered PR list and active successor are unchanged. |
| Can status explain a missing worktree today? | No. `task_status` enters git reconciliation and fails at `git rev-parse --absolute-git-dir`. Resume already has an early read-only missing-worktree refusal. | Let status return durable state without git reconciliation when the directory is absent and reuse the refusal in legal actions. |
| Should status apply the repair automatically? | The task requires post-land inspection without mutating stranded Tasks, and an implicit read-time write would hide the consequential transition. | Status is read-only; explicit resume owns the one safe data migration. |
| Is a schema draft enough for the live recovery? | Draft migrations are accumulated for the next release cut and are not applied by an ordinary development build. | Put the live repair at the explicit resume boundary. A later release migration may sweep untouched legacy rows, but is not required for legal status/resume agreement. |
| Why did new-Task reads fail before the body existed? | Task/PR publication committed before `create_from_placement_plan`; no durable state distinguished that intended gap from a deleted worktree. Worktree creation eventually succeeded, proving this was initialization rather than loss. | Add a transactionally published initialization event and make all three read surfaces consume it. |
| Can rejection still churn under a resident? | Yes. A log-only launch refusal is retried on every Project supervision tick, while recording a fresh generic body-loss event first creates durable noise. | Record the lifecycle refusal once before PR reconciliation and make the exact receipt idempotent across ticks. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Restore a builtin `task` alias to `slice` | Old rows launch without changing data. | It makes invalid durable state look valid, hides catalog drift, and is explicitly excluded. |
| Rewrite every unavailable flow to the current Project lifecycle | Automatically converges all rows. | A removed repo-local flow may encode deliberate behavior, and Project defaults can change after Task creation. Guessing would silently change authored execution. |
| Apply `task → slice` during status/store open | Fleet rows converge before command dispatch. | A read mutates the exact Tasks being inspected, obscures the transition, and conflicts with the requirement to verify LOO-193 without touching it. |
| Add `lf task migrate-lifecycle` as a separate command | Makes mutation maximally explicit. | It leaves status unable to recommend the ordinary recovery command and adds a second operator step for a mapping Loopflow can prove safely. Resume is already the requested transition boundary. |
| Create the git worktree before publishing Task identity | Readers see only fully placed Tasks. | Concurrent starts need the durable Task uniqueness reservation first; reversing the order can leave an unowned worktree when another starter wins. Explicit initialization preserves that ownership boundary. |
| Add `Initializing` to the cross-language `WorkStatus` DTO | Initialization becomes a generic Work lifecycle state. | Only worktree-backed Task placement has this gap. A typed Task event and action reason represent the real boundary without inventing a Wave/Project state or forcing an unrelated DTO migration. |

## Key decisions

- A lifecycle pin is executable proof, not merely a non-empty name. Validation
  covers expansion and phase legality as well as lookup.
- Only the historical default `task` is migratable, and only to `slice` after
  validating the entire candidate plan. Unknown missing references fail closed.
- `resume` performs the migration before any mutable adoption/reconciliation or
  Run reservation. If migration persistence fails, nothing launches.
- Status remains read-only. Its recommendation models what the command will
  legally do, including a named pending migration.
- Automatic supervisors validate but do not migrate. This prevents failure
  storms while keeping the repair an authored lifecycle action.
- Missing worktrees remain the same durable Task. Status exposes recovery; it
  never silently creates a replacement checkout or PR.
- `WorktreeInitializing` is committed with Task identity; `PrStarted` means the
  declared worktree now exists. Initialization is a bounded Task placement
  fact, not a generic Work status.
- Invalid lifecycle non-convergence is one durable event. The resident remains
  responsible for observing it, but has no legal retry until the plan changes.

## Scope

- In scope: Task lifecycle preflight; exact `task → slice` resume migration;
  action/command agreement; missing-worktree status; preservation proofs for
  dirty work, provider continuity, lifecycle cursor, and serial PR history;
  transactional Task worktree-initialization evidence across status, wait, and
  roadmap; stable resident non-convergence; new Task creation validation.
- Out of scope: compatibility aliases; arbitrary flow renames; rebuilding an
  absent worktree automatically; changing Project lifecycle defaults; retry
  budgets and atomic Run failure receipts already owned by the broader recovery
  work; Project/Wave lifecycle migration.

## Done when

- A focused integration fixture reproduces LOO-193: loop phase, no current Run,
  dirty active worktree, `task` loop pin, provider session, failed latest
  attempt, and `resume` recommendation. Resume changes only the loop flow to
  `slice`, reserves a Run through a stub body, and preserves dirty contents,
  Task id, provider session, lifecycle cursor, and PR chain.
- The same proof covers a Task with merged predecessors and an active successor.
- Status on an absent worktree succeeds, recommends `no_action`, names the exact
  path/branch recovery, and leaves the Task/PR rows unchanged.
- A Task launch request with an unavailable lifecycle flow persists no Task or
  PR.
- Repeated automatic launch attempts with an invalid plan create no Runs.
- Two resident ticks over copied LOO-167, LOO-193, and LOO-195 missing-flow
  shapes produce one actionable failure event each, zero Runs, and no PR
  mutation.
- A published `WorktreeInitializing` Task with no directory keeps `lf task
  status`, zero-timeout `lf task wait`, and `lf roadmap --wave ... --json`
  readable; each exposes `no_action` and the initialization reason, and no Run
  exists yet.
- `cargo test -p loopflow --test managed_task_lifecycle_tests` passes.
- `cargo test -p loopflow task_lifecycle`, `cargo fmt --check`, and
  `cargo clippy -p loopflow -- -D warnings` pass.

## Measure

Before: the copied LOO-193 fixture records a new failed Run per resume and
status recommends that failure. After: invalid automatic attempts reserve zero
Runs; the one explicit legacy resume records one repaired plan and one Run.
Operational success is zero `flow not found: task` Task failures after the
repair ships.
