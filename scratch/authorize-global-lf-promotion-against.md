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

Promotion instead **copies** the candidate into a **content-addressed**,
immutable store path and points the global symlink at *that*:

```
~/.lf/bin/lf-<sha256-of-binary-bytes>   # 0o555, never rewritten in place
~/.local/bin/lf -> ~/.lf/bin/lf-<digest>
```

The path is the byte digest of the staged binary, **not** `source_revision`: a
dirty build appends `-dirty` but two dirty builds at one sha differ byte-for-byte,
and a published vs validation-only build at the same sha are distinct binaries —
`lf-<source_revision>` would collide across all three. Content addressing makes
reuse safe and idempotent: if the digest path already exists it is verified
byte-for-byte and reused; a byte mismatch at an existing path is a hard refusal,
and a retained rollback artifact is **never overwritten**. `source_revision`,
authority, and package version ride the preview and the rollback metadata, not
the filename.

The previously-pointed binary path is retained as the rollback candidate. A
worktree rebuild now changes only `local-bin/`, never the global — branch-local
builds are isolated by default, which is Outcome line 1.

**Rollback is revalidated and may refuse.** `lf install rollback` re-runs the
full preflight against the retained binary before repointing (memory: "validate
its latest-known migration against the live store before activation; version
labels and source freshness alone do not prove compatibility"). If a
frontier-advancing migration ran since that binary was promoted (the
`PromoteAndMigrate` path), the retained older binary no longer recognizes the
advanced store, so rollback **fails closed** with the same
`UnknownApplied(version)` reason a forward promotion would — rollback is not a
privileged escape from compatibility. Stated plainly so no operator expects a
guaranteed undo after a migration.

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
`Reserved`** — the lease is the *sole* authority (`ChildLeaseState`,
child_session.rs:190). A `Reserved` lease is a body that has claimed a
generation but not yet booted (memory: new boots strand *before* mark-booted),
so it counts as live. Projected Task/Project status may **enrich** the report
but must never veto the lease fence: `is_process_active()` is not part of the
gate. A revoked/finished/legacy lease is not a live body. Conservative bias is
correct: a false "live" only delays a promotion; a false "dead" is the disaster
we are removing.

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

### `lf install` dispatches before home routing, journal, and store open

`main` today runs `home::route` (bin/lf.rs:1391) and then wraps most commands in
`in_repo_runtime`, which emits a run-`started` journal event (bin/lf.rs:439) that
opens the store. A candidate that does not know the live frontier would fail
*there* — during trace/store capture — never reaching the preflight, which is
exactly the 2026-07-17 symptom (`lf code-review` died in trace capture). So
`Commands::Install` is matched **immediately after CLI parse, before
`home::route` and outside `in_repo_runtime`** (alongside the direct-dispatch
commands like `Desktop`). It performs no journal emission and opens the store
only through its own read-only preflight. A regression drives a candidate that
does not know the frontier and asserts it reaches the **preflight refusal**, not
a store-capture error.

### The launch/promotion fence (shared vs exclusive)

A promotion-only lock plus an under-lock recount is not enough: a new
old-binary body could reserve a generation between the recount and a migration
write, recreating the incompatible-writer incident. So the fence spans both the
promotion and the reservation:

- **Reservation takes `LOCK_SH`** on `~/.lf/promotion.lock`, held only across the
  lease-reserving CAS. The two choke points are the async wrappers
  `store.reserve_task_process` / `store.reserve_project_process`
  (store/child_sessions.rs:221, :1028) — every launch, successor rotation, and
  recovery reservation funnels through them, so wrapping the CAS covers all
  callers. The blocking `flock` runs on `spawn_blocking` to stay off the async
  runtime.
- **Promotion takes `LOCK_EX`** on the same file across its whole critical
  section. While it holds exclusive, no reservation can acquire shared, so no new
  body can start; while reservations hold shared, promotion waits.

This makes "recount live bodies, then act" atomic against launches, not merely
narrowed. A regression holds the promotion `LOCK_EX` at a failpoint, attempts a
`reserve_task_process` concurrently, and asserts it blocks until promotion
releases — the exact interval the reviewer named.

### Publish: stage, then ordered failure-preserving commits

The old "one atomic all-target operation" claim was false — a symlink swap, a
directory `rmtree`+`copytree`, and a skill sync cannot be one atomic op. The
honest contract is **stage everything beside its destination, then commit in a
fixed order where each commit is individually atomic and failure-preserving**,
holding `LOCK_EX` throughout steps 1–4:

1. **Re-check** frontier + live-body count under the lock (TOCTOU close).
2. **Migrate (only `PromoteAndMigrate`, only zero live bodies):** apply the one
   pending migration via `apply_sqlite_with_backup` (its own migration lock +
   backup), nested under the promotion lock.
3. **Stage** (no destination mutated): copy candidate → its content-addressed
   path + `fsync`; build the new CLI symlink at a temp name in the same dir;
   `copytree` the app + bundled helper to a temp dir beside `/Applications` +
   `fsync`.
4. **Commit, in order, each atomic:**
   - *Commit A (CLI, safety-critical, first):* `rename` the temp symlink over
     `~/.local/bin/lf`. `rename` never leaves the target absent — unlike the
     current `_promote`, which `unlink`s then `symlink_to`s (a crash between
     leaves no global `lf`).
   - *Commit B (app):* `rename` the existing app aside to a `.superseded`
     sidecar, `rename` the staged app into place, then remove the sidecar.
   - Record the prior CLI target as the rollback candidate at the moment of
     Commit A.
5. **Post-commit, best-effort (not part of the atomicity claim):** `sync-skills`,
   matching install.py's existing contract that a skill-sync failure warns and
   does not fail the install.

Failpoints are injectable after each commit stage so the regression asserts the
invariant at every torn state: after a failure at any stage, targets are either
fully old or fully new *for that stage*, and there is never a window with no
global `lf`. If Commit B fails after Commit A, the CLI is correct and the app is
left recoverable (staged temp + sidecar both present) and the failure is
reported — the two are genuinely separate commits, not one lie about atomicity.

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
| Can a body launch between the under-lock recount and the migration write? | Only via `store.reserve_task_process`/`reserve_project_process` (store/child_sessions.rs:221, :1028) — a single choke point per kind. | Reservation takes `LOCK_SH` on `promotion.lock`; promotion's `LOCK_EX` blocks it for the whole critical section. Fence, not a narrowed window. |
| Where must `lf install` dispatch to avoid the store-capture failure? | `home::route` (bin/lf.rs:1391) and `in_repo_runtime`'s run-`started` journal emit (bin/lf.rs:439) both precede command bodies and open the store. | Match `Commands::Install` right after CLI parse, before `home::route`, outside `in_repo_runtime`; it opens the store only read-only in its own preflight. |
| Does `lf-<source_revision>` uniquely name a binary? | No — `-dirty` collapses distinct dirty builds, and published vs validation-only at one sha are different binaries. | Name by sha256 of the binary bytes; verify-and-reuse on hit, refuse on byte mismatch, never overwrite a rollback artifact. |

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
3. **The lease fence is the sole live-body authority** (Active/Reserved), for all
   global replacement; status only enriches the report. Named refusals.
4. **A shared/exclusive fence spans promotion *and* reservation**, so the recount
   is atomic against a launch, not merely narrowed by an under-lock recheck.
5. **`lf install` dispatches before home routing, journal, and store open**, so a
   frontier-incompatible candidate reaches the preflight refusal, not a
   store-capture crash.
6. **Reuse `store::migrations` and the ledger verbatim; zero Python migration
   knowledge.** The reject reasons are the store's own error strings.
7. **Fail closed** on any missing/unreadable evidence, before any target moves.
8. **Staged, ordered, failure-preserving publish** (CLI commit first, app second,
   skill sync post-commit best-effort) with a content-addressed immutable binary
   and a preserved, re-validated rollback that may itself refuse after a
   migration.

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
- the exact-frontier validation-only candidate **promotes** and retains a
  rollback binary;
- the canonical candidate **applies `N+1` only with no live bodies** and retains
  a rollback binary;
- a **`Reserved` lease with no live process** blocks promotion — the lease
  fence is not vetoed by projected status (boundary 3);
- a `reserve_task_process` attempted while promotion holds `LOCK_EX` at a
  failpoint **blocks** until promotion releases, and cannot slip a body in before
  the migration (boundary 1);
- a candidate that **does not know frontier `N`** reaches the **preflight
  refusal**, not a trace/store-capture error, because `install` dispatched before
  home/journal/store-open (boundary 4);
- injecting a **failpoint after Commit A (CLI) but before Commit B (app)** leaves
  a correct global `lf` and a recoverable app, never a torn state with no global
  `lf`; a failpoint after staging but before Commit A leaves every target
  unchanged (boundary 2);
- `lf install rollback` after the `N+1` migration **refuses** with
  `UnknownApplied` because the retained binary predates the advanced frontier
  (boundary 6);
- two byte-distinct builds do **not** collide on `~/.lf/bin/`, and re-promoting an
  identical binary reuses its digest path without overwriting a retained rollback
  (boundary 5);
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
