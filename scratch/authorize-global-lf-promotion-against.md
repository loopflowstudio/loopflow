# Authorize global `lf` promotion against the shared migration frontier

## Problem

On 2026-07-17 a human ran `uv run python scripts/install.py local --use` from
`loopflow.lf-docs`. That command silently repointed the machine-global
`~/.local/bin/lf` at a branch binary whose migration registry ended at
`0.11.026`, while the shared production store `~/.lf/loopflow.db` was already at
`0.11.027`. Every subsequent `lf` invocation across the fleet — including live
Task and Project bodies mid-turn — hit a store its own binary could not read
(`migration 0.11.027_accounts_first is unknown to lf`). Recovery was a manual
symlink repoint to a retained pre-replacement executable.

The defect is not the `--use` capability and not the human — it is that a
high-blast-radius global replacement performed **no preview, no compatibility
check, and no live-body check** before mutating the one command the whole
machine depends on. `scripts/install.py._promote` (install.py:361) unlinks and
symlinks the global CLI and `rmtree`s `/Applications/Loopflow.app` with zero
knowledge of the store frontier or of running bodies. Python cannot safely own
that decision: the migration registry and the Task/Project ledger are Rust, and
duplicating either in Python guarantees drift.

Beneficiaries: every human and agent on the machine. A promotion that cannot
strand live bodies is a Developer-Efficiency KR ("No Task strands on a dead
body"; "zero manual git/setup surgery").

## The demo

From a branch whose `lf` is behind the shared store's migration frontier
(reproduces 2026-07-17 exactly):

```
$ uv run python scripts/install.py local --use
Building lf (cargo release)...
Promotion preflight (candidate 8c60a8e6, validation-only):
  current CLI     ~/.local/bin/lf -> ~/.lf/bin/lf-3e9df067  (0.11.027, published)
  candidate       local-bin/lf     (source 8c60a8e6, validation-only)
  shared store    ~/.lf/loopflow.db  frontier 0.11.027_accounts_first
  candidate knows through 0.11.026_lineage_boundary
  live bodies     2 (W2-320 ts_a56be8a6, Developer Efficiency ps_1db1324d)
  REFUSED: candidate does not recognize applied migration
           0.11.027_accounts_first; promoting it would make the shared store
           unreadable to the global lf. And 2 live bodies make any global
           replacement unsafe.
  ~/.local/bin/lf and /Applications/Loopflow.app are unchanged.
$ echo $?
1
$ lf --version    # still the working global; the fleet never broke
```

The same command from a branch **at** the frontier, with bodies drained,
promotes atomically and prints the retained rollback target.

## Approach

Split promotion into two owners along the line the task draws:

- **Python stages artifacts.** `install.py` keeps building, bundling, code
  signing, and writing into the worktree's `local-bin/`. It loses all authority
  to `unlink`, `copy`, `symlink`, or `rmtree` a machine-global target. `_promote`
  is deleted.
- **Rust owns promotion policy and the mutation.** A new command
  `lf install <preflight|promote|rollback>`, **run by the freshly built
  candidate binary itself**, decides compatibility and liveness against the
  Rust migration registry and the shared ledger, then performs the atomic swap.

The candidate binary running its own promotion is the load-bearing choice: it
already carries its own `MIGRATIONS`, its own `migration_authority()`, its own
`build_info::source_revision()`, and the exact `validate_sqlite` /
`pending_migrations` logic that the store uses at open time. Running the
candidate against the shared store read-only **is** the compatibility test —
the same code path that would fail later, run now, before anything moves.

Python invokes it after staging:

```python
# install.py, replacing _promote(...)
candidate = LOCAL_BIN / "lf"          # local --use
# or ROOT / "target/release/lf"       # refresh
subprocess.run([str(candidate), "install", "promote",
                "--cli-target", str(install_dir / "lf"),
                "--app-source", str(LOCAL_BIN / f"{APP_NAME}.app"),
                "--app-target", str(APPLICATIONS_DIR / f"{APP_NAME}.app"),
                *(["--preview"] if dry_run else [])],
               check=True)
```

Rust reads its *own* identity (it is the candidate); Python only names *where*
the artifacts are and *where* they go. Rust never parses a migration file or
infers schema — it calls the same store functions the runtime already trusts.

### Immutable, copy-based promotion (kills the root hazard)

The current `_promote` symlinks the global `lf` **into the worktree's
`local-bin/`** "so rebuilds take effect with no extra skill." That convenience
is the deeper W2-319 hazard: a later `cargo build` in that worktree silently
rebuilds the global binary — an implicit fleet replacement with no promotion at
all (wave memory, 2026-07-17: "An active symlink into a mutable worktree makes a
later local build an implicit fleet replacement").

Promotion instead **copies** the candidate into a content-addressed, immutable
store path and points the global symlink at *that*:

```
~/.lf/bin/lf-<source_revision>      # 0o555, never rewritten in place
~/.local/bin/lf -> ~/.lf/bin/lf-<source_revision>
```

The previously-pointed binary is retained as the rollback candidate. A worktree
rebuild now changes only `local-bin/`, never the global — branch-local builds
are isolated by default, which is Outcome line 1. `lf install rollback`
re-runs the same preflight against the retained binary before repointing
(memory: "validate its latest-known migration against the live store before
activation; version labels and source freshness alone do not prove
compatibility").

## The promotion decision

Read the shared store frontier read-only and classify the candidate against it,
reusing `store::migrations` verbatim (`latest_applied_version_sqlite`,
`pending_migrations`, `validate_sqlite`, `validate_applied_checksums`). This is
exactly `doctor::inspect_store`, lifted into a shared preflight function and
extended with a live-body count and the target/rollback paths.

| Candidate vs store frontier | Authority | Decision |
|---|---|---|
| Store carries a migration the candidate does **not** know (candidate older/divergent) | any | **Reject** — naming the unknown applied version and the candidate's latest known (the 2026-07-17 case) |
| Applied checksum mismatch, or divergent branch history in the store | any | **Reject** with the exact version and both checksums |
| Candidate recognizes the store **exactly**, no pending migration (frontier == candidate latest) | validation-only **or** published | **Promote** (no store write) |
| Candidate is ahead: has pending migration(s) the store lacks | validation-only | **Reject** — a branch build must not advance the shared store |
| Candidate is ahead with a compatible pending migration | published (canonical main / tagged) | **Promote and apply** via `apply_sqlite_with_backup`, but only with **no live bodies** |
| Store frontier unreadable, missing, or evidence incomplete | any | **Reject, fail closed**, before any replacement |

Orthogonal to the table and checked for **every** promotion:

- **Any live Task or Project body ⇒ reject**, and name each one (issue id +
  session id). "Global replacement is unsafe while a body is live" is the
  primary safety gate; the migration-apply case is a strictly stronger form of
  it. Promotion never stops bodies — the operator drains them.

Live = a Task/Project session whose current write **lease state is `Active` or
`Reserved`** and whose status `is_process_active()`. The lease is the
authoritative fencing token (`ChildLeaseState`, child_session.rs:190); status
alone is too coarse (wave memory repeatedly: parked sessions read
`is_process_active` while their body is dead). A revoked/finished/legacy lease
is not a live body. Conservative bias is correct: a false "live" only delays a
promotion; a false "dead" is the disaster we are removing.

Compatibility guarantees the residual race is benign: because we only ever
promote a candidate that recognizes the frontier, a body launched in the
microsecond after the under-lock re-check runs a compatible global either way.
The live-body gate exists to avoid yanking the binary out from under a *running*
turn, not to prove a compatible swap safe.

## The promotion boundary (structured, one owner)

`lf install preflight --json` emits a read-only `PromotionPreview`:

```rust
struct PromotionPreview {
    candidate: CandidateIdentity,     // source_revision, source_identity, authority, package_version
    cli_target: TargetState,          // path, current link -> resolved binary, its frontier+authority
    app_target: TargetState,
    store: StoreFrontier,             // db path, latest_applied, candidate latest_known
    compatibility: Compatibility,     // Exact | AheadPending(Vec<version>) | UnknownApplied(version) |
                                      //   ChecksumMismatch{version,..} | Divergent | Unreadable(reason)
    live_bodies: Vec<LiveBody>,       // {kind, issue, session_id, generation}
    rollback_retained: Option<String>,// path kept if promotion succeeds
    verdict: Verdict,                 // Promote | PromoteAndMigrate | Reject(Vec<Reason>)
}
```

`promote` computes the same preview, prints it (human) or `--json`, and — unless
`--preview` — proceeds to publish. Python renders whatever the boundary prints;
it makes no decision. Every global entry point
(`local --use`, `refresh`, `refresh --no-pull`, `--install-dir`, the app, the
bundled `lf`) routes through this one function, so the checks live in exactly
one place.

### Publish is atomic and locked

Hold one exclusive `flock` on `~/.lf/promotion.lock` across the whole
critical section:

1. Re-read frontier and live-body count **under the lock** (closes the
   preview→publish TOCTOU).
2. If `PromoteAndMigrate`: apply pending migration via `apply_sqlite_with_backup`
   (which takes its own migration lock and writes the backup) — still holding
   the promotion lock, still gated on zero live bodies.
3. Copy candidate → `~/.lf/bin/lf-<rev>` (0o555), `fsync`, atomic-`rename` the
   `~/.local/bin/lf` symlink via a temp link, `rmtree`+`copytree` the app +
   bundled helper, then `sync-skills`. CLI, app, bundled helper, and skills are
   one accepted operation.
4. Record the prior CLI target as the rollback candidate.

Any failure leaves every target byte-for-byte / link-for-link unchanged: build
the new symlink at a temp name and `rename` it over the old only after the copy
`fsync`s; never `unlink` the live target first (the current `_promote` unlinks
then symlinks — a crash between them leaves no global `lf` at all).

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does the candidate binary already know its own identity + registry? | Yes: `build_info::{source_revision,source_identity,migration_authority}` and `store::migrations::MIGRATIONS`/`latest_known_version` are compiled in; `install.py` already stamps `LOOPFLOW_MIGRATION_AUTHORITY` per build (install.py:176–201, build.rs:113). | Run promotion *as the candidate*; no cross-binary RPC, no re-derivation. |
| Can a possibly-older candidate read the shared frontier without mutating it? | Yes: `doctor::inspect_store` opens the store `SQLITE_OPEN_READ_ONLY` and calls `latest_applied_version_sqlite` + `validate_sqlite`; the "unknown migration" error is produced read-only (migrations.rs:988). | The preflight is a read-only lift of `inspect_store` + liveness; no new store-open semantics. |
| Is the migration-compatibility logic reusable as-is? | `pending_migrations` (migrations.rs:978) already returns the exact "unknown to lf … latest known …" error; `validate_applied_checksums` (772) catches checksum drift; `divergent_history` (925) catches the one known divergence. | Reject reasons come straight from these; Python parses nothing. |
| How is body liveness authoritatively read? | `ChildLeaseState::{Active,Reserved}` is the live fencing state; `list_task_sessions`/`list_project_sessions` (child_sessions.rs:825, 2509) enumerate sessions; `status.is_process_active()` is the coarse cross-check. | Live = active/reserved lease + process-active status; name each in the refusal. |
| Does `may_apply_migrations` already fence validation-only writes to production? | Yes (store/mod.rs:255): a validation-only build against `~/.lf/loopflow.db` gets `validate_sqlite` (read-only), never `apply`. | The store already refuses the *write*; W2-319 is that promotion mutates the **global command** before any store open happens. Promotion must gate the swap, not the store write. |
| Will "no replacement while any body is live" make promotion impossible during dogfooding? | Likely blocks often; but that is the intended, safe behavior — the refusal names each body so the operator drains a quiet window. The task is explicit: "Do not stop live bodies as part of promotion." | Accept the operational cost; document it; refusal is actionable, not a dead end. |
| Does copy-vs-symlink break the "rebuild takes effect" workflow? | Yes — intentionally. That auto-effect is the implicit-fleet-replacement footgun. Developers who want the new build re-run `--use` (which now previews). | Key decision, called out below; net safety win. |
| Is there a promotion-lock deadlock with the migration lock? | Promotion lock is a distinct file (`promotion.lock`); migration application nests the migration lock under it, always in that order; no other path takes both. | No cycle. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep policy in Python; parse migration dirs / query the store from Python | No new Rust command | Second registry guaranteed to drift; the store's checksum/divergent/lineage logic is 400 lines of Rust that Python would half-reimplement. Task forbids it explicitly. |
| Have the *running* installer's own `lf` (not the candidate) do the check | One fewer exec hop | The running `lf` may be a different binary than the candidate; only the candidate's own `MIGRATIONS` answers "can *this* binary operate the store." |
| Gate only the migration-apply case on live bodies; allow same-frontier swaps under live bodies | Fewer refusals | Still yanks the binary from under a running turn on a filesystem level; the 2026-07-17 incident had bodies live at swap. The blanket gate is simpler and matches the Outcome. |
| Keep symlink-into-`local-bin`; add a preflight only | Smaller diff | Leaves the implicit-fleet-replacement footgun fully intact; a later `cargo build` still replaces the fleet with no preflight. |
| Stop live bodies as part of promotion | "Just works" | Explicitly excluded; promotion is not authorized to kill work. |

## Key decisions

1. **The candidate binary runs its own promotion.** Compatibility is "can *this*
   binary read/operate the store," which only the candidate's compiled-in
   registry can answer. Python execs the freshly-built candidate.
2. **Promote a copied, immutable binary — not a symlink into a worktree.**
   Isolates branch builds by default and removes the implicit-fleet-replacement
   hazard that was W2-319's mechanism. Cost: `--use` no longer auto-applies later
   rebuilds; re-run `--use` (now safe) to re-promote.
3. **One blanket live-body gate for all global replacement**, stricter for the
   migration-apply case (which additionally writes the store). Named refusals.
4. **Reuse `store::migrations` and the ledger verbatim; zero Python migration
   knowledge.** The reject reasons are the store's own error strings.
5. **Fail closed** on any missing/unreadable evidence, before any target moves.
6. **Atomic, lock-guarded publish** with a preserved, re-validated rollback.

## Scope

- **In scope:** delete `install.py._promote`; route `local --use`, `refresh`,
  `refresh --no-pull`, `--install-dir`, app + bundled-helper install, and
  post-install skill sync through `lf install {preflight,promote,rollback}`; the
  Rust promotion boundary (preflight struct, decision, atomic locked publish,
  rollback retention); copy-based immutable global binary; the two-worktree
  end-to-end regression; docs on local-vs-global builds.
- **Out of scope:** stopping live bodies; OS-level immutability flags or bin-dir
  permission changes; defending against stale pre-change scripts that unlink the
  bin directly (an old worktree must update before it can promote); any change
  to shipped migrations, migration ordering, or the migration guard; the
  remote-release path (`lf release` → CI).

## Done when

A deterministic two-worktree regression (Rust, driving real `install`/store
code, fake bodies via the lease store) builds:

1. a shared store advanced by a newer published build (frontier `N`),
2. an older/divergent validation-only candidate (knows `< N`),
3. a validation-only candidate exactly at frontier `N`,
4. a canonical published candidate with one pending migration `N+1`,
5. a live Task/Project body (active lease).

and proves:

- every global entry point **rejects** the older/divergent candidate and the
  live-body case **before** the CLI or app target changes (assert targets are
  byte-for-byte / link-for-link identical after the refusal);
- the exact-frontier validation-only candidate **promotes atomically** and
  retains a rollback binary;
- the canonical candidate **applies `N+1` only with no live bodies** and retains
  a rollback binary;
- **sabotaging any supported path to call the old direct copy/symlink** (i.e.
  reintroducing `_promote`'s unlink+symlink, or bypassing the boundary) makes the
  regression fail — the guard is load-bearing, not decorative.

Plus: `uv run python scripts/install.py local --use` from a behind-frontier
branch prints the refusal and leaves `~/.local/bin/lf` and
`/Applications/Loopflow.app` unchanged (the demo).

## Measure

- **Avoidable global-promotion incidents:** baseline 1 (2026-07-17,
  fleet-wide). Target 0 — no promotion that leaves the shared store unreadable
  to the global `lf`, and no promotion under a live body, across a month of real
  `--use`/`refresh` runs.
- **Refusal legibility:** every refusal names the exact database frontier,
  candidate revision, and each live body — checked by the regression's asserted
  refusal strings.
