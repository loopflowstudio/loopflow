# Delete Session, make Run the sole executor

## Problem

PR #1073 made Run the durable *authority* but left it sharing the road with the
thing it replaced. Project and Task execution still runs through
`project_session/runner.rs` and `task/runner.rs`: they reserve, activate,
revoke, reap, and settle their own bodies against `ProjectSessionStatus` /
`TaskSessionStatus` and a `u32` generation, then **mirror** those transitions
into Run via `reserve_run_for_child` / `activate_run_for_child`.

Two controller stacks means every execution question has two answers, and the
mirror is where they diverge. A Run can be Active while its Session says
revoked. Recovery reconstructs authority from ambient env instead of evidence.
`generation: u32` is threaded through 359 call sites as a de-facto identity
that Run already owns properly. The Wave resident, meanwhile, never got the
Project runner's child-first control lane, so a child waiting on its parent can
sit behind background cadence.

Who benefits: anyone steering real work. The bugs this shape produces —
stale completion committing over live input, a dead process retaining write
authority, an interrupt landing on the wrong body — are exactly the silent
stalls the Loopflow API project's KRs are measured on.

Why now: the spine exists and is proven. Carrying the bridge another release
means every new behavior gets built twice.

## The demo

Start a real Task in the live Product Wave, steer it mid-flight, interrupt it,
`kill -9` its controller process, and watch the keeper adopt the work into a
fresh Run with provider continuity intact and the outstanding Steer still
outstanding — then let it finish and land its PR. Throughout:

```
$ lf runs --json | jq '.[0] | {run, launch, work}'
{"run":"run_…","launch":"lnc_…","work":{"task":"tsk_…"}}

$ sqlite3 ~/.lf/lfd.db 'select count(*) from task_sessions'
0
```

No Session row is created, read, or recovered at any point. That is done-when
#7, performed rather than asserted.

## Approach

One shared Run execution path keyed by `WorkRef`, with typed dispatch only
where domain behavior genuinely differs.

```text
                    ┌─ shared ────────────────────────────────┐
  reserve_run ─────▶│ reserve · advance · stop · reap · recover │
                    └──────────────┬──────────────────────────┘
                                   │ match WorkRef
              ┌────────────────────┼────────────────────┐
              ▼                    ▼                    ▼
         Wave policy         Project policy         Task policy
      chat/cadence/          KRs, child Task      workspace, PR,
      project selection        judgment            CI, impl flow
```

The shared path owns: reservation, lease, boundary Basis, control-lane drain,
containment proof, stop, reap, recovery, and completion fencing. The domain
policies own: which flow runs, what closure means, what effects are legal. No
factory trait, no registry, no generic Work row — an explicit `match` on
`WorkRef`, per `docs/architecture.md`'s identity decision.

### Refinement after reading the three loops (recorded during implementation)

"One shared path" is **two** shared pieces, not one mega event loop. Reading
`flowloop/wave.rs`, `project_session/runner.rs`, and `task/runner.rs` shows the
Wave loop is a *supervisor* that spawns body processes through `BodyBackend`,
while the Project and Task runners *are* bodies driving a provider harness
in-process. Collapsing those two shapes into one function would be a category
error. The duplication is real, but it sits in two layers:

**(A) Supervisor — reserve, launch, probe, reap, recover.** Duplicated across
`ops/project.rs` (`reserve_project_session`, `launch_project_process`) and
`ops/task.rs` (`task_run`/`task_start`, `launch_task_process:1779`,
`recover_stranded_task_body:2246`, `recover_stalled_task_body:2352`). This is
what done-when #2's "one Run reserve/advance/stop/recover path" names. Wave
already runs its own version of this shape against `Option<WaveControl>`.

**(B) Body — the harness driver loop.** `project_session/runner.rs` and
`task/runner.rs` are structurally *identical* here: the same `tokio::select!`
arms (stdin attachment; a poll tick running `absorb_run_control` then
`send_outstanding_steers` then child-attention preemption; `event_rx` with the
same `ConversationEvent` match). Task's is larger only because its
`TurnCompleted` arm also carries PR/CI/gate settlement. This is the bigger
deletion — 1,232 + 2,991 lines collapsing toward one loop.

So the target is `supervise_work(WorkRef)` and `run_work_body(WorkRef)`, with
typed `match` dispatch at exactly four domain boundaries inside the body loop:
prepare-next-step, `TurnCompleted` settlement, closure check, and allowed
effects. Everything else is shared.

Launch already has homes for every Session field the body loop persists:
`Launch.resume_token` takes `session.provider_session_id`,
`Launch.containment` takes `latest_process.tmux_name`, `Launch.opaque_basis`
takes the TUI boundary. The one missing store op is an
`observe_launch_provider(lease, launch, resume_token, process_group)` — Launch
rows are currently written once by `insert_control_launch` and never updated.

`task/actions.rs` is the wedge. `derive_task_actions(&TaskActionEvidence) ->
TaskActionModel` is already pure: evidence in, decision out, no store. Re-point
it at Run/Launch evidence **first**. It proves the spine carries the decision
logic before a single controller loop is deleted, and it is fully testable
without a process.

### Progress ledger

- [x] **Stage 0 — measurable baseline.** `scripts/measure_source.py` commits the
      metric; `rust/loopflow/src` = 144,210 at `ae1344a57`, reproducible.
- [x] **Launch owns provider continuity.** `observe_launch_provider` under the
      Run lease. This was the last capability Session had that Run/Launch could
      not express, so it gated everything else. Containment stays immutable
      (spawn-time fencing evidence); three behavioral tests including the
      stopped-Run refusal.
- [x] **Stage 1 — evidence port.** `TaskActionEvidence.status` is now
      `WorkStatus`, derived from Epoch/Run via `store.work_status(&work)` at
      both production call sites. The pure decision function no longer reads a
      Session enum. 26 tests pass over 17,280 exhaustive combinations.
- [ ] **Stage 2 — shared supervisor + body loop.**
- [ ] **Stage 3 — the cut.**
- [ ] **Stage 4 — surfaces.**
- [ ] **Stage 5 — draft migration + ledger.**

Verified state at the last commit: `cargo test -p loopflow --lib` = **1,581
passed, 0 failed**; `cargo clippy --lib --tests -D warnings` clean; source at
144,462 (+252 from baseline, since stages 0–1 are additive — the size gate
correctly refuses to call that progress).

Commits so far: `aaec96cc4` ledger correction + design · `4b82caddc` measure
script · `73fc72882` `observe_launch_provider` · `c86bb211b` evidence port.

#### Stage 2 shape (next session starts here)

Two extractions, in this order:

1. **Body loop first** — it is the bigger win (1,232 + 2,991 lines) and the two
   loops are already structurally identical. Extract
   `run_work_body(work: WorkRef, lease: &RunLease, …)` holding the
   `tokio::select!` with its four arms (stdin attachment; poll tick →
   `absorb_run_control` → `send_outstanding_steers` → child-attention
   preemption; task-supervision tick; `event_rx`). Typed `match work` at exactly
   four domain boundaries: prepare-next-step, `TurnCompleted` settlement,
   closure check, allowed effects. Session writes inside the loop map to:
   `update_*_session_for_lease` → `observe_launch_provider` (already built);
   `append_*_event_for_lease` → Run/Launch receipts;
   `ChildTarget::Project(&id, lease)` → `&run_lease`.
2. **Supervisor second** — unify `reserve_project_session` /
   `launch_project_process` with `task_run`/`task_start` /
   `launch_task_process:1779` / `recover_stranded_task_body:2246` /
   `recover_stalled_task_body:2352` onto `reserve_run` + `RunAdvance` +
   `stop_run`, with the containment probe moved verbatim so `Unprovable` stays
   fenced.

Do **not** start stage 3 until both extractions compile and the old runners are
thin wrappers over them — that is what makes the deletion mechanical instead of
a rewrite.

#### Blast-radius survey (measured; corrects the kickoff estimate)

The kickoff called `generation` a 359-reference mechanical wavefront. That was
wrong in a useful direction: **only 12 function signatures take it**, none
returns it, and the bulk of the ~400 token hits are `.generation` field reads
inside code already slated for deletion. Three SQL columns
(`process_generation` ×24 in one file, `lease_generation` ×8 in one file) and
one event variant (`LeaseRevoked`) carry the rest.

Likewise the ~95 production Session write-call-sites **funnel through three
wrappers**: `ops/task.rs:891–995` (the `*_with_authority` family),
`task/runner.rs:1499` (`set_and_record_status`), and
`project_session/runner.rs:942`. Retargeting those three onto Run/Launch covers
most of the mechanical work. (`store/mod.rs`'s 34 apparent hits are entirely
inside `mod tests`, which starts at line 1021.)

`ops/task.rs` is **46% tests** — production is lines 1–5060 only. Of that:
~1,600 deletable Session lifecycle (`task_run` 363–742; `record_task_failure`
…`launch_task_process` 1681–1956; the supervision/recovery block 1957–2463,
the single biggest deletable run; control verbs 4715–5060), ~2,900 must-survive
PR/CI/git/gate/workspace domain, ~550 genuinely mixed. The hardest single
function is `reconcile_task_pr_with_authority` (2734–3056, ~320 lines), which
interleaves PR reconciliation with Session status writes at 2875–2931, 2959,
and 3005–3040.

**Category (d) — needs redesign, not deletion.** These use the Session enum as
a *vocabulary* for real domain policy:

| Site | What it really encodes |
| --- | --- |
| `task/mod.rs:172,179` | `TaskGateProposal` settle-outcome validation — needs its own outcome enum |
| `task/mod.rs:916,924` | PM writeback invariants (completed vs gate cycle) |
| `ops/task.rs:2489` | `decide_open_pr_status` — pure CI-triage policy returning the Session enum; must re-emit as a Run/Wait decision |
| `ops/task.rs:2959,3037` | edge-triggered "merge completes Task" detection; needs a Run-terminal-transition equivalent |
| `project_session/runner.rs:848–878` | Project outcome aggregation over child Task statuses, incl. the `Blocked` fingerprint-stall rule — real supervisory policy |
| `store/sqlite/child_sessions.rs:146,170,613,1044,1080` | recovery-succession and completion-transaction preconditions |
| `store/ci_incidents.rs:16` | **persisted** `task_status` column — needs a migration, not a type swap |

The Session→Epoch bridge (`epoch_state_for_task/project`,
`close_task_epoch_if_quiescent` at `child_sessions.rs:2356/2385` and
`durable.rs:2706/2720`) is category (d) inverted: delete the *mapping*, because
Run must own that seam directly.

#### The category (d) decoupling pattern (established; apply to the rest)

`decide_open_pr_status` is done and is the template. The move:

1. Give the policy its **own** enum naming the real facts
   (`OpenPrDisposition::{ObservationDegraded, NeedsDirection, AwaitingReview}`).
2. Return that from the pure function. Keep the operator-facing reason strings
   byte-identical — they are asserted and user-visible.
3. Translate to `TaskSessionStatus` **at the call site inside the doomed
   runner**, in a clearly-labelled boundary function. That is not a
   compatibility shim: the translation is deleted along with the runner, while
   the enum is what survives onto `WaitOn`.

Why it is worth doing before the loops move: the old enum flattened
"we could not observe CI" and "CI is red and nobody fixed it" into one
`Blocked`, and those need different operator responses — one waits on a
capability recovering, the other needs authored direction. Splitting them is a
behavioral improvement the deletion would otherwise have to invent under
pressure.

**Do not** apply this to `TaskGateProposal.status` (`task/mod.rs:172`) or
`store/ci_incidents.rs:16`. Those are *persisted* fields, so changing their type
is a wire/schema change; they ride the Session row's own deletion in stage 3
and its draft migration, not a separate type swap. Starting one and not
finishing it leaves a half-migrated wire type, which is worse than either end
state.

Stage 1 was far cheaper than estimated: production code branched on
`evidence.status` in exactly **two** places (`is_terminal()` and `== Abandoned`);
the other 36 references were test fixtures. The status axis also shrank 8 → 5,
which is the real simplification — `Created`/`Starting` collapse into `Ready`,
`Blocked` into `Waiting`, and `Failed` stops being a Work state at all because
it is Run health.

### Execution order (one PR, five stages, each compiling)

1. **Evidence port.** `TaskActionEvidence` sources from Run/Launch/Turn instead
   of `TaskSessionStatus`. `derive_task_actions` unchanged in shape. Old runners
   still call it. Behavioral tests pass at both ends.
2. **Shared path.** Add `run_work(WorkRef)` — the single reserve/advance/stop/
   recover loop — next to the old runners. Wave routes through it (it has the
   fewest Session dependencies). Project's child-first control lane, which
   already works, moves here and Wave inherits it.
3. **Cut.** Project and Task route through `run_work`. Delete
   `project_session/`, `task/runner.rs`, `child_session.rs`, `child_control.rs`,
   `ops/child.rs`, and the `store/child_sessions.rs` pair. Delete the six env
   vars and the `generation: u32` parameter everywhere. Stop writing
   `runs.lease_generation` / `runs.source_id`.
4. **Surfaces.** DTO fixtures, Swift, CLI, Python, docs, builtins — same pass,
   because a DTO change that Swift lags on is a broken app.
5. **Schema draft + ledger.** `scripts/new_migration.py drop_session_controller`
   writes the one-way deletion as a draft. Rewrite `docs/architecture.md`'s
   moment-of-transparency to describe reality with no bridge.

Stages 1–2 are additive and independently revertable. Stage 3 is the one-way
door and must be atomic. This is one PR, not a serial chain: splitting after
stage 3 would ship a Rust DTO surface that Swift cannot decode, and splitting
before it would leave exactly the dual controller stack the task forbids.

## De-risking

| Question | Finding | Impact on design |
| --- | --- | --- |
| Is the 121,819-line ceiling reachable? | No, and not reproducible. Deletable pool is ~11,260 whole + ~4,000 trimmed, minus ~2,500 added back → ~131,500. No measurement of the current tree yields 121,818. | Substitute a committed metric + pinned baseline + ≥10,000 net reduction. Flagged in `scratch/questions.md`; needs a human ruling. |
| Does deleting the legacy env vars break fail-closed authority? | **Yes, silently.** `ops/child.rs:369` uses the six vars as the *sentinel* for "I am in a Run". Without them, a lease-less in-Run process falls to `Ok(None)` = User authority. | `LF_RUN_CONTEXT=agent` becomes the sole positive in-Run marker, set at every Launch. Regression test: RUN_CONTEXT present + RUN_LEASE absent ⇒ refuse. |
| Can the branch stop writing `runs.lease_generation` without a migration? | Yes. Both it and `source_id` are nullable and the unique index is partial (`WHERE … IS NOT NULL`). | Stop writing them; index goes inert. **No compatibility shim needed.** Draft migration drops the columns at release. |
| Are drafts applied on this branch? | No — `drafts/` holds only a README and Rust references it nowhere. `canonicalize_migrations.py` runs at `lf release run`. | Done-when #1's "zero tables" = zero *code references*. Physical DROP is a release event. Say so in the ledger. |
| Is `LF_RUN_LEASE` actually plumbed, or aspirational? | Plumbed and fail-closed: set at `ops/project.rs:424` and `ops/task.rs:1893`, resolved at `ops/child.rs:356` via `resolve_run_lease`. | The authority half of the cut is already done. This task removes its *competitor*, not builds its replacement. |
| Does the Run spine already key on `WorkRef`? | Yes. `reserve_run(&WorkRef, &HomeId, RunTrigger)`, `advance_run`, `stop_run`, `done(lease, basis)` in `store/durable.rs`. | No new spine API. The work is routing Project/Task into it and deleting the mirror (`reserve_run_for_child` / `activate_run_for_child`). |
| What is the real blast radius? | `TaskSessionStatus` 362 refs / 20 files; `generation` 359 refs; `ProjectSessionStatus` 122; `ChildWriteLease` 118. | `generation` threading is the mechanical bulk. Mostly signature deletion, not logic change — but it is what makes stage 3 unsplittable. |
| What blocks the test suite from compiling? | `tests/support/mod.rs` references all three status/lease types. | No test target builds until it is migrated. It is the first file in stage 3, not the last. |
| Can containment proof move without weakening? | The `Absent \| Present \| Unprovable` verdict and tmux-vetoes-stale-pid rule live in the child-body probe. | Move the probe verbatim into the shared reaper. Only `Absent` releases the slot — regression test on `Unprovable` staying fenced. |
| Is any "session" a false positive? | Many. `URLSession` (21 hits), tmux `TerminalSession` attach, Anthropic's 5-hour usage window, `provider_session_id`, Claude `--resume`, Context Lab's agent-session grain. | Explicitly preserve. Done-when #1's "focused searches document any surviving use" is satisfied by an inventory, not by zero hits. |

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Serial PRs split by noun (Project first, then Task) | Smaller reviews | Leaves two controller stacks live between PRs — precisely what the task forbids. Both share `child_session.rs`, so neither can delete it alone. |
| Serial PRs split by layer (Rust, then Swift) | Each PR compiles in its own language | Ships a DTO surface Swift cannot decode. Violates done-when #4 for the duration. |
| Keep `TaskSessionStatus` as a derived view over Run | 362 call sites unchanged; tiny diff | The compatibility layer the task exists to prevent. Two names for one fact is how the mirror diverged in the first place. |
| Generic `WorkRuntime` trait with three impls | Feels symmetric | `docs/architecture.md` closes this: no factory, no registry. The domains differ in policy, not lifecycle — a trait would re-introduce the indirection this cut removes. |
| Land the schema drop as a canonical migration | Tables actually gone on-branch | #1073's canonical tail was a declared one-time exception. A second one re-creates the drift the draft contract fixed. |

## Key decisions

**One PR, not a chain.** Defensible only because stages 1–2 are additive and
stage 3 is mechanical (signature deletion) rather than semantic. If stage 3's
diff exceeds review capacity, the fallback is to land stages 1–2 as a first PR
(`lf pr land --next session-cut`) — additive, no dual stack — and make stage 3+
the second. Decide at stage 2's end with the real diff in hand, not now.

**`LF_RUN_CONTEXT` becomes load-bearing.** It is currently decorative. Making
it the in-Run marker means every Launch must set it unconditionally, and a
launch path that forgets it downgrades authority silently. Guard: assert it in
the Launch constructor, not at each call site.

**Domain dispatch is `match`, not polymorphism.** Three arms, explicit,
grep-able. When Wave and Project both need the child-first lane and Task does
not, that asymmetry should be *visible* in the match, not hidden behind a
default trait method.

**Keeper recovery adopts through evidence, never identity.** No reconstructing
a controller from Handles. The keeper sees a Reserved Run that missed boot, or
an Active Run whose Launch containment is `Absent`, and reserves a successor
linked by `retry_of`. `Unprovable` stays fenced — an unreachable Home is not a
dead Run.

**Preserve vendor and tmux "session" vocabulary.** Renaming `URLSession` or
Claude's `--resume` session id would be cargo-culting the deletion. The word is
only wrong when it names a Loopflow controller.

## Scope

**In scope**
- Shared `run_work(WorkRef)` reserve/advance/stop/reap/recover path.
- Deleting `project_session/`, `task/runner.rs`, `child_session.rs`,
  `child_control.rs`, `ops/child.rs`, `store/child_sessions.rs` +
  `store/sqlite/child_sessions.rs`, and the Run mirror functions.
- Deleting `ProjectSessionStatus`, `TaskSessionStatus`, `ChildWriteLease`,
  `ChildProcessGeneration`, the `generation: u32` thread, and the six env vars.
- Wave inheriting Project's child-first control lane.
- DTO fixtures, Swift, CLI, Python, docs, builtins in the same pass.
- The draft deletion migration.
- `docs/architecture.md` rewritten to describe reality with no bridge.

**Out of scope**
- Provider resume continuity, `provider_session_id`, Claude `--resume`.
- tmux `TerminalSession` attach presentation and Ghostty surface keying
  (rename-only where the parameter name misleads).
- Context Lab's agent-session observability grain.
- Anthropic 5-hour usage window labelling.
- The four architecture open questions unrelated to this cut (OpenCode usage
  normalization, `HomeId` migration source, opaque-TUI handback mechanism,
  native-subagent containment per provider).
- Reaching 121,819 lines — see `scratch/questions.md`.

## Done when

```bash
# 1. zero controller references; survivors are vendor/tmux/UI only
rg -n 'ProjectSessionStatus|TaskSessionStatus|ChildWriteLease|ChildProcessGeneration' rust/ swift/ python/   # → 0
rg -n 'LF_(PROJECT|TASK)_(SESSION_ID|GENERATION|LEASE_TOKEN)' rust/ swift/ python/ scripts/                  # → 0
rg -n 'reserve_run_for_child|activate_run_for_child'                                                          # → 0

# 2. one execution path
rg -n 'fn run_project_session|fn run_task_session'                                                            # → 0

# 4. everything compiles and passes
cargo fmt --check && cargo clippy -- -D warnings && cargo test -p loopflow
cargo test -p loopflow --test dto_fixtures
uv run pytest python/tests/ && scripts/test.py --swift

# 6. net reduction (substitute criterion — see questions.md)
uv run python scripts/measure_source.py --baseline 144210 --min-reduction 10000
```

Plus the behavioral proofs of done-when #3, as deterministic-barrier tests (no
sleeps), per the ledger's normative race list:

- exact-authority: a stale lease cannot advance or complete;
- stale completion: Steer commits between boundary and `done` ⇒ proposal rejected;
- input-vs-stop: input arriving during `stop` resolves into the next Run, never lost;
- portability: identical durable outcome for live-send (Codex), seed-only
  (Claude one-shot), and opaque tmux TUI;
- keeper adoption: Reserved-but-never-booted Run adopted; `Unprovable`
  containment stays fenced; only `Absent` releases the slot;
- restart: Work, Epoch, worktree, PR, outstanding Steers, and child attention
  survive a controller `kill -9`;
- child-first: a Wave servicing an awaiting child Review before background
  cadence, resuming its playhead without replaying completed steps.

And #7: the demo above, performed against the live Product Wave.

## Measure

| Metric | Baseline (`ae1344a57`) | Target |
| --- | --- | --- |
| `rust/loopflow/src` physical lines | 144,210 | ≤ 134,210 (−10,000, deletion-sourced) |
| Execution controller loops | 3 (wave, project, task) | 1 |
| `generation` references | 359 | 0 |
| `TaskSessionStatus` references | 362 | 0 |
| Session-authority env vars | 6 | 0 |

Capture the baseline with the committed `scripts/measure_source.py` in stage 1
so the reduction is measured, not estimated.

## Wave alignment

Serves the Loopflow API project's KR *"One model everywhere, continuously —
a parallel concept appearing anywhere is a failure event."* The Session
controller **is** the parallel concept; deleting it is the KR's proof, not a
step toward it. Also serves *"Task loops earn trust by streak: zero silent
stalls"* — the mirror between Session and Run is where silent stalls originate.

New risk introduced: `LF_RUN_CONTEXT` becomes load-bearing for authority. A
Launch path that omits it downgrades an agent to User authority silently.
Mitigated by setting it in the Launch constructor and testing the refusal.
Worth a wave-memory entry once proven.
