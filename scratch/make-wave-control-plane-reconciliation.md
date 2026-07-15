# W2-169 — Self-healing Wave control-plane reconciliation

Linear: W2-169 · Project: Loopflow API · Wave: product · Task Session
`ts_f9757c8d0ba848a48d9943122a59dd16`.

## Problem, in one screen

A live Wave does not converge its durable intent with the world. Five distinct
failures, each with a concrete code owner:

1. **No periodic convergence.** A live wave runs a `StoreObserver`, a
   `BusListener`, and a resident `Supervisor` (`wave/mod.rs:296-342`). The
   `Supervisor` watches only the **resident process** (`wave/supervisor.rs:27`
   doc: "never touches a vendor"). Child Project/Task bodies are reconciled only
   lazily, on a read or command path — `reconcile_project_liveness`
   (`ops/project.rs:548`) is called from `project_status` (`:627`) and snapshot
   reads, never on a timer. So a resident that never reads a dead Project never
   heals it.
2. **`reconcile_project_liveness` marks failure; it never recovers.** On a
   vanished body it reaps and sets `Failed` (`ops/project.rs:596-604`). It does
   not attempt to adopt a live body or start a safe missing one. "Self-healing"
   does not exist today — the best case is "correctly marked dead."
3. **Active counts count intent, not liveness.** `snapshot_wave` computes
   `active_projects`/`active_tasks` as `list_*_sessions(wave).filter(|s|
   !s.status.is_terminal()).count()` (`lf/commands/waves.rs:874-887`). A dead
   body whose Session is still `Running`/`Waiting` is counted active. This is
   evidence #2 (five active Projects, every body dead).
4. **PR truth is only as fresh as the last live loop tick.** `TaskPr.phase`
   (`task/mod.rs:201-218`) is durable but reconciled from `gh` **only inside**
   `reconcile_task_pr*` in the live loops (`task/runner.rs:333`,
   `project_session/runner.rs:772`). `lf status`/`lf roadmap` do no git/network
   (`waves.rs:484-488`). A stopped Task shows its last-observed phase — a merged
   PR reads open (evidence #3, W2-135 / PR #903).
5. **Read-only inspection mutates canonical main.** `lf wt list` →
   `wt_list` → `sync_main` (`lf/commands/ops/mod.rs:1204`) → `git fetch` +
   `reset_worktree_to` (`engine/git.rs:461,506`): `git stash push
   --include-untracked` + `git reset --hard origin/main` + conditional `git
   stash pop`. Meanwhile `ensure_clean_main` (`ops/project.rs:352`) gates every
   Project/Wave turn on that same checkout being clean. So an inspection command
   perturbs the exact state control-plane turns require (evidence #4, #5). No
   `ReadOnly`/`is_mutating` command classification exists (only routing-oriented
   `is_routable`, `lf/commands/home.rs:121`, which does not even include `Wt`).

There is also a **latent asset**: the single body projection `observe(...) ->
BodyObservation` (`child_session.rs:710`) — the W2-123/W2-135 "every surface
reads one shape" reducer — exists with unit tests and is **wired to nothing**.
The shipped attention/section/next-owner derivation is a parallel ad-hoc
computation in `waves.rs` (`RoadmapSection`, `next_move_for_task:1298`). Two
projections from the same primitives; unifying is part of this Task, not net-new
modeling.

## User-visible outcome

A running Wave continuously converges durable Project/Task intent with live
bodies, provider work, PR truth, and repo state. Users stop seeing dead Projects
counted active, merged PRs described open, or read commands rewriting their
checkout. After body death or laptop restart, Loopflow either recovers the same
work exactly once or names the exact boundary needing intervention with durable
evidence.

## Source of truth

Durable Wave/Project/Task/directive/PR-sequence/body-generation records in the
Loopflow store own **intent and identity**. Process probes (`tmux`), GitHub
(`gh`), Git, and provider sessions are **timestamped observations**, not
alternate identity. One reconciliation projection derives status, active counts,
attention, next owner, and legal controls for every surface. That projection is
`child_session::observe -> BodyObservation`; consumers stop re-deriving.

## Deterministic reproductions (build these first, keep all five)

Each is a Rust test (or a scripted repro under `scratch/repro/` where a live
wave is needed) that fails on `main` and passes at Task end. Do **not** collapse
them.

- **R1 — no periodic recovery.** With a wave "served," a selected Project whose
  body tmux session is killed stays `Running` and is never reaped until a read
  path touches it. Assert: a bounded convergence tick (once wired) transitions
  it. On `main`: no tick exists → fails.
- **R2 — recover vs mark-dead.** Given a reap-safe dead body (non-terminal
  intent, no live tmux, replay-safe), reconciliation adopts/relaunches exactly
  once rather than only stamping `Failed`. On `main` `reconcile_project_liveness`
  only sets `Failed` → fails.
- **R3 — count semantics.** A wave with N selected Projects all of whose bodies
  are dead reports `live_bodies == 0` while `desired_active` stays N. On `main`
  the single `active_projects` field conflates them → fails.
- **R4 — PR staleness.** A Task with a durable `Open` phase whose PR `gh`
  reports `MERGED` reconciles to `Merged` and advances the PR sequence exactly
  once from an observation, without a manual status probe, and wakes the owning
  decision path. On `main` this only happens inside a live loop tick → fails when
  the loop is stopped.
- **R5 — side-effect-free inspection.** `lf wt list` (and status/roadmap/doctor)
  leaves HEAD, index, worktree, refs, and stash byte-for-byte unchanged. On
  `main` `wt_list` calls `sync_main` → fails (fetch + reset + stash observable).

R5 is the cheapest and most mechanical — land its fix early to unblock clean
dogfooding of R1–R4 (today you cannot even inspect without perturbing main).

## Slice plan (serial PRs, one worktree)

Boundaries may shift when the code proves a simpler sequence.

### Slice 1 — side-effect-free inspection + explicit sync API (R5) — LANDED (PR 1)
Smallest, unblocks everything. What shipped:
- `wt list` is read-only by default. `WtCommand::List` already carried a `sync:
  bool` flag that `run_wt` dropped (`{ format, .. }`) and `wt_list` ignored,
  calling `sync_main` unconditionally. Now `run_wt` threads it and `wt_list`
  only calls `sync_main` when `--sync` is passed — the explicit, self-owned
  mutation ("the command names and owns that mutation explicitly"). Default
  reads reflect the last-synced main; merge/fresh labels tolerate that.
- **Mechanical guard = behavioral boundary test, not a classifier enum.** A
  `CommandEffect`/`is_read_only` marker consumed only by a test is exactly the
  maintained-list ceremony the repo rejects (structural over enforcement; gates
  rejected 3× in one day). Instead `wt_list_leaves_canonical_main_byte_for_byte_
  unchanged` drives the real `lf` binary against a repo with origin ahead of
  local + a dirty edit and asserts HEAD/porcelain/stash/refs are identical —
  verified to fail on the unconditional-sync behavior. `wt_list_sync_flag_owns_
  the_fast_forward` pins that `--sync` still fetches+fast-forwards. If a future
  inspection command regains a transitive `sync_main`, the analogous test bites.
- **Deferred within this slice:** a broader boundary test over `status`,
  `roadmap`, `doctor`, `project/task status`, `diff`. Exploration confirmed none
  call `sync_main` today (only routing-oriented `is_routable`, `home.rs:121`,
  exists — and it excludes `Wt`), so `wt list` was the sole live bug. Add the
  cross-command test if a cheap harness for those commands appears; noted in
  `scratch/questions.md`.

### Slice 2 — reconciliation source-of-truth model
Wire the latent projection to consumers so status stops re-deriving.
- Route `waves.rs` `RoadmapSection`/`NextMove`/attention through
  `BodyObservation` (`child_session.rs:710`) rather than `TaskSessionStatus ×
  liveness × PrPhase`. Keep the wire DTO shape stable (round-trip fixtures under
  `tests/fixtures/dto/`); this is an internal derivation change.
- Define **distinct typed fields**: `desired_active` (intent: non-terminal
  Sessions) vs `live_bodies` (observed: bodies passing the liveness probe).
  Counts state which they count; a dead body can never make a live count
  positive (R3). This is a DTO field addition → update fixture + Rust + Swift +
  Python round-trip together (DTO rule: no defaults, explicit fields).
- Verify: R3 test green; `roadmap_snapshot.json` fixture carries both counts;
  Swift `DTOFixtureTests` pass.

### Slice 3 — periodic Project-body recovery + active-count semantics (R1, R2)
Give reconciliation a periodic owner and real recovery.
- Add a per-wave convergence tick next to `StoreObserver` (`wave/mod.rs:305`,
  `wave/registry.rs` `poll_once:116`). It enumerates this wave's non-terminal
  Project (and Task) Sessions and runs the liveness/`observe` projection.
- Extend recovery beyond "mark Failed": on a dead reap-safe body, **adopt** a
  live matching generation if present, else **relaunch** via
  `launch_project_process` (`ops/project.rs:403`) / the `LaunchIntent::Supervisor`
  path (`ops/child.rs:443`) which already encodes "a supervisor may not restart
  delivered/abandoning work." Terminal intent is preserved; unsafe recovery
  becomes `NeedsInput` with evidence, never a silent relaunch. Honor
  `supervisor_restart_bar` (`project_session/mod.rs:148`) — the W2-135 lesson
  (submitted/interrupted/abandoned work is never revived).
- Idempotent + restart-safe: fencing via `ChildProcessGeneration` /
  `begin_generation` (`project_session/mod.rs:187`) guarantees exactly-once
  adoption under concurrent ticks; a second tick is a no-op.
- Verify: R1 + R2 tests green; a killed body is recovered exactly once; a
  submitted PR is never revived (regression from the 2026-07-14 W2-129 incident).

### Slice 4 — PR observation convergence (R4) — SPLIT: landing/completion → W2-171
**Scope partition (2026-07-15 steer).** Infrastructure launched **W2-171** to own
the *narrow landing/completion repair*: out-of-band merged `TaskPr` reconciliation
plus the `.lf/tmp/scratch-stash` path. W2-169 must **not** edit the same
completion/landing slice (`reconcile_task_pr_with_authority`'s merge→settle/
complete transition). W2-169 keeps the **read-side** and **model** aspects:
truthful active counts, read-side PR freshness, the broader durable observation
model, periodic Project recovery, and side-effect-free inspection.

- **Handed to W2-171 (do not re-implement here):** the stable-PR-identity /
  append-only fix for the reused-branch overwrite (a merged publication clobbered
  by a later closed empty draft on the reused serial branch). A complete,
  bite-verified implementation is preserved at commit **`bcb11c6cc`** (adds
  `pr_by_number` in `ops/pr.rs`; makes `reconcile_task_pr_with_authority` observe
  the bound PR by number, never rebinding `publication.github` to a different
  number; test `reused_branch_probe_does_not_overwrite_a_bound_publication`).
  W2-171 can cherry-pick it. It was reverted from this branch to keep the
  landing/completion slice single-owner.
- **Remains W2-169 (read-side freshness only):** let `lf status`/`roadmap` reflect
  merge/PR truth without a live loop tick, off the shared observation model
  (Slice 2), preserving the last `gh` observation with its age on GitHub failure
  and stating freshness — **without** mutating the merge→settle/complete
  transition W2-171 owns. Coordinate the shared `reconcile_task_pr_with_authority`
  seam with W2-171 (they land the identity/settlement change; W2-169 consumes it).

### Slice 5 — shared consumers + real two-wave dogfood (end-to-end proof)
- Ensure CLI (`lf status`/`roadmap`/`project|task status`) and Swift render the
  same state/reason/freshness/owner/controls off the one projection. Every
  automatic action and refusal emits durable evidence feeding W2-123 (no second
  presentation model).
- Run the seed's end-to-end proof (below) across process + laptop restart.

## End-to-end proof

Product and Infrastructure each with ≥2 selected Projects. Kill every Project
body, merge one waiting Task PR externally, leave canonical main dirty with named
uncommitted edits, invoke `wt list` + `status` + `roadmap` + Project
reconciliation concurrently. Prove: residents recover every replay-safe body
exactly once; `live_bodies`/`desired_active` stay distinct and correct; the
merged PR advances once and reaches its decision path; all inspection commands
leave HEAD/index/worktree/refs/stash byte-for-byte unchanged; unsafe recovery
becomes `NeedsInput` with evidence; CLI and Swift fixtures render identically.
Repeat across restart; re-running reconciliation is a no-op.

## Absent & error states

Missing execution context, unavailable GitHub, unreachable Home, ambiguous
process ownership, dirty canonical checkout, stale observations, uncertain
external side effects, conflicting generations — each stays explicit evidence.
Never guess, report an empty healthy state, mutate main during inspection, or
replay a control-uncertain action. A Session predating execution-context pinning
stays `NeedsInput` (the W2-135 legacy-lease lesson), not force-relaunched.

## Operational boundary

Bounded, no provider body required for mechanical adoption, liveness, GitHub
refresh, or status projection. Read-only local commands return promptly from
persisted evidence + bounded probes. GitHub failure preserves the last
observation with its age. A restart converges without a human noticing the stall.

## Decisions taken (reversible; simpler path)

- **Convergence tick lives beside `StoreObserver`, not in the resident.** The
  resident is vendor-shaped and can die; reconciliation is store-shaped and must
  outlive any body. Reuse the existing per-wave poller rather than a new process.
- **`observe`/`BodyObservation` is the single projection.** Reuse the proven
  W2-123 reducer; do not mint a second. `waves.rs` re-expresses on top of it.
- **Read-only guard is structural (compile/test-time), not runtime.** Matches the
  repo's "make violations unspellable" bias; a runtime effect-check is rejected.
- **Counts are two fields, not a relabeled one.** `desired_active` +
  `live_bodies`; DTO addition mirrored in all four languages in one commit.

## Exclusions

No stash pop/delete, no discarding dirty work, no second supervisor/store, no
GitHub review-policy redesign. Canonical main stays read-only to Project turns —
recovery does not make it writable to bypass the boundary. Build on W2-135
generations/leases, W2-140 succession, W2-156 CI observation, W2-123 visible
state; do not duplicate their models. A missing primitive is extended in its
owning shared API within this serial sequence, with the dependency recorded.

## Open question (non-blocking)

If R2/R3/R4's convergence tick needs a cap distinct from the generic 8-pass /
2-hour task defaults (per the "project-loop caps" open fork in MEMORY), pick a
conservative bounded default in Slice 3 and note the dogfood data needed to tune
it, rather than blocking the design on it.
