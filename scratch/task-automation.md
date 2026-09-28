# Task automation through verified landing

> Archived launch design for LOO-332. Implementation and current evidence belong
> to `/Users/jack/src/loopflow.scheduled-task-operation/scratch/task-automation.md`.
> The execution instructions below record the original handoff.

2026-09-28 · Jack Heart · Product · Launch-plan core

## What to build

Keep selected Tasks progressing through their intended Flows to verified landing
and closure, using desktop-managed finite scheduled operations instead of a
resident or a PR watcher. Jack's contract: "make sure things get to landed
successfully (but dont skip intended steps etc)".

Builds on [Data model: one table per user object · LOO-298](https://linear.app/loopflow/issue/LOO-298/data-model-one-table-per-user-object-session-as-a-child-of-run).
Use its current AgentSession/FlowSession/Exec direction and shared claims. Its
implementation is still active; settle integration against the inherited source,
not the older installed runtime. Do not change LOO-298's checkout or live Home.

This is the ambitious single-threaded core. Internal slices ship as one coherent
PR. Discord delivery and VSM policy are separate outcomes; preserve their
requirements in `discord.md` without building them in this PR.

## Demo

Enable Task automation for a repository in Desktop, close the app, and observe
one eligible Task start its intended Flow. A second Task waiting for human review
stays parked. The first reaches `pr land -c`, which returns without a watcher.
Later OS ticks repair failed CI or a required rebase, observe actual merge, and
close the Task in Loopflow and Linear. Reopen Desktop to see the last check,
current work and any blocker. Live deployment remains an explicit demo action;
local builds use private fixtures and never migrate the installed Home.

## Current owners and deletion

- `ops/cron.rs` and `lf/commands/ops/mod.rs`: Wave-specific daily launchd jobs
  and receipts; generalize to the finite repository operation.
- `controller/task/mod.rs`, `ops/task.rs`, shared Flow driver: preserve execution
  order, claims, recovery and review boundaries.
- `ops/pr_landing.rs`: remove the persistent supervisor/wait loop; retain useful
  observations, incident identity and saved merge disposition.
- `ops/task::settle_task_landing`: reuse Task closure and PR-chain settlement.
- `ci-fix` and `pr-land` builtin skills: replace watcher assumptions with the
  finite handoff. `ci-fix` already starts with rebase and returns after arm.
- Desktop invokes shared Rust/CLI operations; no Swift-owned scheduling policy.

## Domain values and operations

Use existing Task, AgentSession, FlowSession, PR, landing intent and CI incident
records. Extend their actual owners only where needed for elapsed CI wait,
repair admission and pending settlement. No second Task cursor or public runtime
object. A schedule records repository, cadence and placed Home; its latest
receipt distinguishes a successful idle check from failed observation.

Proposed internal API shape (adapt names to LOO-298's final types):

```rust
async fn reconcile_repository(store: &Store, repo: &Path) -> Result<TickReport>;
async fn reconcile_task(store: &Store, task: &Task) -> Result<TaskAction>;
fn sync_schedule(spec: &ScheduleSpec) -> Result<InstalledSchedule>;
```

`TaskAction` distinguishes wait, launch/resume, repair, settlement and unresolved
evidence. These are results of existing domain operations, not persistent plans.
Final exported DTOs are explicit required/optional fields with shared fixtures.

## Admission and execution

Use one repository OS job, initially every minute, with per-Task claims. This
cadence and job arrangement are launch-plan defaults, not additional decisions
attributed to Jack. Make the schedule adjustable using the existing scheduling
surface. A tick ends after inspecting and admitting useful work; no waiting loop.

Eligible work is a Task explicitly started/selected for automation or carrying
pending authorized delivery. Unstarted backlog, explicit holds, closed Tasks
and pending human Sessions stay untouched. A Task-bound active Session, Flow
driver or repair prevents a competing launch. Unknown liveness stays unresolved.
Saved but interrupted Flow state is recoverable, not proof of active execution.
Respect Home placement; another Home's work must not be started locally.

When idle, task-operate selects the appropriate Flow only if none is selected,
otherwise resumes/reconciles existing work. It hands execution to the shared
driver and exits. Explicit Flow selections, captured order and review gates
survive every tick. Admission and handoff share authoritative claims: a
preflight read alone cannot prevent two simultaneous launches. Include a test
where a review opens during admission. Admission code must not block its own
already-authorized operator from handing execution to the Flow.

## Landing, repair and closure

`pr land -c` prepares/publishes, requests authorized auto-merge, records
completion-on-merge, and returns. Bare land keeps the Task open; `--next`
preserves the PR-chain disposition. Success means handoff, never false merge.
Later Flow steps requiring an actual merge must remain parked until that fact
exists. Pending delivery remains discoverable after every initiating process
exits. Avoid duplicating `land` and `arm` lifecycle implementations.

The finite check acts on the current PR/head:

| Evidence | Action |
| --- | --- |
| Required checks failed | Admit `ci-fix` when Task admission permits |
| Rebase required, even if green | Admit `ci-fix` immediately; it rebases and resolves conflicts |
| CI pending/queued/absent beyond 30 minutes | Admit diagnosis through `ci-fix` |
| CI pending below deadline | Wait without an agent |
| Green; review or merge pending | Wait or apply only the already-authorized merge action |
| Provider observation unavailable | Keep the exact gap visible; do not infer failure/merge |
| Merged | Apply saved settlement intent without an agent |
| Closed without merge | Keep unresolved; never complete as successful |

CI wait timing survives ticks/restarts and includes expected checks that never
start. New head or explicit CI rerun starts a new attempt without deleting
history. An unchanged incident cannot launch duplicate repairs. A failed repair
with an unchanged external blocker stays visibly blocked until new evidence or
explicit retry. Bound consecutive timeout-only reruns; begin with one automatic
rerun per incident before surfacing the blocker. Do not invent commits to reset
the timer. Green-but-needs-rebase remains actionable independent of this clock.

`ci-fix` rebases, repairs, verifies, publishes and restores the saved disposition,
then returns. It does not restart the feature Flow or skip its earlier reviews.
Task closure retries partial local/Linear settlement after authoritative merge.
Merged-but-closure-pending remains distinct from complete. Repeated settlement
converges without restarting implementation or duplicating completed effects.

Existing taskless landing requests also need a finite observation path: inspect
recorded repository landing intents with their existing PR claims, but perform
no Task closure. Do not retain a watcher solely for taskless callers.

## Internal slices

1. **This slice: finite reconcile and landing handoff.** Reuse observations,
   claims and settlement; prove `land -c` returns and a later invocation repairs
   or closes correctly. Preserve generic Flow merge-dependent boundaries.
2. Add Task admission and task-operate handoff; prove active/review exclusions,
   recovery and overlapping ticks against the real shared driver.
3. Install/manage the repository job through Desktop and shared `lf` operations;
   expose enabled state, cadence and latest outcome/error. Reopening the app or
   resyncing must not duplicate jobs; disabling stops new admissions.
4. Update user docs and builtin delivery skills, run affected checks once, and
   demonstrate the full installed path through the Flow's human demo gate.

## Done when / forbidden near-misses

Prove busy Tasks, open human Sessions and terminal Tasks never get a competing
driver; two ticks admit at most one operator/repair. Prove deadline persistence,
rebase despite green CI, stale-head rejection, no repeated unchanged repair,
and interrupted settlement recovery. Then prove one installed job makes progress
with Desktop closed, including merge followed by Task closure after `land`
has exited. Mocks are useful focused evidence, not that installed demonstration.

No lfd, Wave resident, replacement keeper, watcher, parallel Flow cursor,
automatic review approval, Swift lifecycle implementation, or accidental
automation of unselected backlog. A published PR, green CI, an exited Flow or
a launched cron alone does not satisfy this core.

## Evidence ledger and handoff

Source design was checkpointed at `ce0263277` in `loopflow.discord`.
The attempted direct rebase encountered unrelated main commits #1307/#1308;
it was aborted intact. Prepare this core directly stacked on LOO-298 through
`lf task prepare --stack-on`; record the actual inherited commit before launch.
No implementation or live schedule activation was performed by launch-plan.
Implementation should append dated proof here while preserving the full scope.
