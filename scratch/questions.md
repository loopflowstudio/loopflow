# Open questions / assumptions — W2-319

Headless run: proceeding on best judgment; these are the calls a reviewer might
reopen.

**Resolved by review ir_8095…** (kept for the record):
- Immutable-copy direction confirmed; content-addressed by byte digest.
- Lease (Active/Reserved) is the sole live-body authority; status may not veto.
- Promotion fence must also block reservation (shared/exclusive on the same
  lock); publish contract rewritten to staged ordered commits; `install`
  dispatches before home/journal/store-open; rollback revalidates and may refuse.

## Implementation sequencing (serial PRs)

The approved design lands over two serial PRs on this Task branch — the change is
too large to write and verify blind in one, and this host cannot run cargo
locally (syspolicyd wedge), so each PR must be independently CI-verifiable.

- **PR1 — MERGED as `879ceb930` (#1074):** the read-only preflight boundary —
  `lf install preflight [--json]`, the pure `decide()` core +
  `Compatibility`/`Verdict` types, read-only frontier classification (reusing
  `store::migrations` verbatim), lease-based live-body count, dispatched before
  home/journal/store open. Fully unit-tested; every actionable CI leaf green.
  Now on `main` (`rust/loopflow/src/lf/commands/install.rs`,
  `build_info.rs` Serialize on `MigrationAuthority`, `bin/lf.rs` early dispatch).
- **PR2 — THIS branch (`lf-install-promote`):** the mutating half — `lf install
  {promote,rollback}` (content-addressed immutable binary, staged ordered
  failure-preserving commits, `LOCK_EX`), the reservation `LOCK_SH` launch
  fence, Python routing (delete `install.py._promote`), and the two-worktree
  regression. PR1's merged `decide()` is the source of truth PR2 consumes; the
  live-body *gate* already ships in `decide()`, so PR2 adds only the
  launch-during-promotion fence, the publish mechanism, and the Python cutover.
  This branch carries `scratch/` (design) again, so `scratch-clear` is
  structurally red until PR2 lands — not a repair item.

  PR2 remains serial relative to later app/Python work, but Project review made
  the CLI activation boundary indivisible: the mutating command, Task+Project
  reservation fence, immutable prior retention, candidate staging, CLI commit,
  and migration ordering must share one head.
  - **2a+2c (this review iteration):** `lf install promote --cli-target <p>
    [--preview]` — the CLI half. Content-addressed immutable binary
    (`~/.lf/bin/lf-<sha256>`, verify-and-reuse, refuse byte mismatch), exclusive
    `~/.lf/promotion.lock` held across the under-lock decide + swap, matching
    `LOCK_SH` across both reservation CASes, atomic temp-symlink→rename commit
    (never leaves the target absent), and immutable rollback bytes retained for
    both symlink and regular-file targets. Candidate and prior bytes are staged
    before the candidate becomes global; only then may `PromoteAndMigrate`
    advance the store via `SqliteStore::new`. Thus a post-migration failure
    always leaves a frontier-compatible global command. Filesystem tests cover
    content addressing and both prior target shapes; the fence regression proves
    Task and Project reservations block behind `LOCK_EX`.
  - **2b:** the app-bundle swap (`--app-source`/`--app-target`, `.superseded`
    sidecar) + post-commit best-effort `sync-skills`, and `lf install rollback`
    (re-exec the retained binary's `preflight --json`, refuse on `Reject`).
  - **2d:** Python cutover — delete `install.py._promote`, route `local --use`/
    `refresh`/`--install-dir`/app through `lf install promote` — plus the
    two-worktree end-to-end regression and the sabotage guard.

Still open / reviewable:

1. **Blanket live-body gate obstructiveness.** Any Active/Reserved lease blocks
   *all* global replacement. Given continuous dogfooding this will refuse often.
   **Assumption:** intended safety; the refusal names each body to drain a quiet
   window. The narrower rule (gate only frontier-advancing promotions) is a
   one-line relaxation but reopens the "swap under a running turn" window the
   incident hit — not taking it.

2. **Command surface name.** Chose `lf install {preflight,promote,rollback}` (new
   top-level `Commands::Install`). If `lf promote`/`lf self` is preferred it is a
   rename only.

3. **Reservation fence latency.** `LOCK_SH` is held only across the lease CAS via
   `spawn_blocking`, so steady-state contention is negligible; a promotion holding
   `LOCK_EX` briefly stalls new launches for the duration of its critical section
   (bounded by the migration/backup, seconds). Assumed acceptable; flagged in case
   the migration backup is large enough to want a launch-side timeout+retry.
