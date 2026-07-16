# Make trace capture references survive cleanup and interrupted runs

## Problem

`lf doctor --json` on the published v0.11.2 release reports `capture=fail: 235
failure(s)` because 235 `agent_launches` rows carry `capture_status = 'complete'`
while their `conversation.jsonl` artifacts are gone from disk. The Home health
surface is therefore permanently red, and — worse — a fresh capture bug that
drops one new file would be invisible: one more failure among 235 reads as noise.

The task framed four candidate causes (capture write, cleanup, interrupted runs,
retention). Research **eliminated three of them** and reframed the fourth:

- **Capture write cannot strand a `complete` reference.** `TraceCapture::begin`
  (`trace.rs:913`) writes `conversation.jsonl` into a hidden staging dir, fsyncs,
  then atomically `fs::rename`s it into `artifact_dir` *before* inserting the DB
  row (`trace.rs:970`, insert at `:1071`). `capture_status` starts `capturing`
  and only becomes `complete` in `finish()` *after* a second fsync
  (`trace.rs:1305-1326`). The file always exists before, and at, the moment the
  row claims `complete`.
- **Interrupted runs don't produce missing files.** A run killed before
  `finish()` leaves `capture_status = 'capturing'` with the file present (it was
  created at `begin`). That is a *stuck-capturing* row, not a *dangling-file*
  row. Doctor only flags it if the process also emitted a terminal run event.
- **No cleanup / retention / GC path exists at all.** Nothing in the tree deletes
  trace files or `agent_launches` rows. The only two `remove_dir_all`/`remove_file`
  calls under `~/.lf/traces` are insert-rollback compensation (`trace.rs:1074`,
  `:1179`) that fire only when the DB insert *fails* — they can't strand a row.
  Worktree/session rotation removes sibling git worktrees, never `~/.lf/traces`.
  The only retention in the codebase is the bus sweeper and git-worktree
  autoprune. Trace capture (landed in #871) has never had a lifecycle.

So the 235 references are **whole run trees removed out of band** — verified: the
run directories named in the failure detail (`79cf91df…`, `80bff511…`, `9e14e2e8…`,
`1df1aa0d…`) are entirely absent from `~/.lf/traces`, not just their conversation
files. Cause: manual cleanup, disk reclaim, or a store copied from another host.
Loopflow has **no concept of a retained-but-gone capture**, so any external
removal becomes a permanent, indistinguishable-from-fresh-loss failure.

Research also surfaced a **latent second bug**: `trace_root()` (`trace.rs:1357`)
honors only `LF_HOME`, else hardcodes `~/.lf/traces`. The store home
(`store::lf_home_dir`, `store/mod.rs:138`) honors `LF_CONTROL_HOME`, build
provenance, and the dev/release split (`~/.lf-dev/worktrees/<id>` for dev). When
those diverge — a dev build, or a release run under `LF_CONTROL_HOME` — the
capture-time root and the doctor-time root differ, and a file that is intact
under one root reads as missing under the other. This manufactures **false**
dangling references and, left unfixed, would let reconciliation tombstone files
that actually exist.

## The demo

```bash
# 1. Copy the production-shaped store + traces to a scratch home (never touch prod).
cp ~/.lf/loopflow.db /tmp/w2-235/loopflow.db && cp -R ~/.lf/traces /tmp/w2-235/traces
LF_HOME=/tmp/w2-235 lf doctor --json | jq '.checks[] | select(.name=="capture")'
#   → "status":"fail","detail":"235 failure(s); … conversation.jsonl: No such file …"

# 2. Reconcile: acknowledge the out-of-band loss (dry-run first, then apply).
LF_HOME=/tmp/w2-235 lf runs reconcile              # reports 235 missing terminal captures
LF_HOME=/tmp/w2-235 lf runs reconcile --apply      # tombstones them → pruned

# 3. Doctor is green, and the count is now honest.
LF_HOME=/tmp/w2-235 lf doctor --json | jq '.checks[] | select(.name=="capture")'
#   → "status":"ok","detail":"… 235 pruned …"

# 4. Prove fresh loss still fails: delete one live capture, re-run doctor.
rm /tmp/w2-235/traces/<live-run>/…/conversation.jsonl
LF_HOME=/tmp/w2-235 lf doctor --json | jq '.checks[] | select(.name=="capture")'
#   → "status":"fail","detail":"1 failure(s); …"   (fresh, un-acknowledged loss)
```

The win: a maintainer turns a permanently-red 235 into `ok` with one explicit
command, and the surface stays sensitive to genuinely new capture loss.

## Approach

A retained `agent_launches` row is in exactly one capture state, and every
terminal state has a hard contract:

| `capture_status` | Meaning | Artifact contract | Doctor |
|---|---|---|---|
| `capturing` | live, file growing | file exists | fail only if process ended (existing) |
| `complete` / `partial` / `prompt_only` | terminal, captured | file **must** resolve + read | missing file ⇒ **fail (fresh loss)** |
| `pruned` *(new)* | terminal tombstone | file known-absent, absence acknowledged | counted, **not** a failure |

Three changes deliver this:

### 1. Add the `pruned` terminal state (migration + write path)

New forward migration, allocated as the **next free ordinal after rebasing onto
main** (`.018` is owned by PR #986 `session_body_provenance` and `.020` by #1010
`task_pr_linear_linkage`; main also carries `.019_task_pr_github_observation`, so
the next mechanical ordinal is **`.021`** → `0.11.021_capture_pruned_state.sql`).
Allocate via `scripts/new_migration.py` post-rebase so the ordinal reflects
main's actual high-water mark, never a hardcoded guess. SQLite bakes CHECK
constraints into the table, so widen the enum with the documented rebuild (mirror
`0.11.002_project_session_successors.sql`): create `agent_launches_next` with
`capture_status … CHECK (capture_status IN ('capturing','complete','partial','prompt_only','pruned'))`,
`INSERT … SELECT` all rows, drop, rename, recreate the five indexes
(`idx_agent_launches_run/process/wave/project/task`). The migration runner already disables FK
actions and runs `foreign_key_check` around the transaction, so the
`agent_turns.launch_id … ON DELETE CASCADE` reference is safe.

`pruned` reuses the existing `incomplete_reason` column to carry cause + a
timestamp, e.g. `"conversation artifact absent at reconcile 2026-07-15T…Z"` or
`"pruned by retention <policy>"`. No new column.

### 2. `lf runs reconcile [--apply]` — the operator-driven tombstone verb

Lives in `runs.rs` (already the trace-facing surface: `lf runs trace <id>`).
Capture lifecycle is W2-235's ownership; keeping the mutation here — not in
read-only `doctor` — preserves doctor as a pure audit and keeps W2-236's shared
classification code untouched.

Behavior (dry-run by default; `--apply` writes; idempotent):

- Scan every terminal launch (`complete`/`partial`/`prompt_only`). For each whose
  `conversation_path` does not resolve to an existing readable file, mark it a
  **pruned candidate**.
- **Age guard against masking fresh bugs:** by default only tombstone candidates
  whose `ended_at` is older than 48h; report recent ones separately as
  `"N recent missing captures — investigate before reconciling"`. `--all` overrides.
  This is the line that keeps a live capture regression from being silently swept.
- Also finalize **orphaned `capturing`** launches — process provably dead (not
  live, run event terminal or start older than the guard) — to `partial` with
  reason `"capture interrupted; process ended without finalizing"`, making
  interrupted runs terminal and honest.
- `--apply` performs each transition as a single `UPDATE agent_launches SET
  capture_status=?, incomplete_reason=?, ended_at=COALESCE(ended_at,?) WHERE id=?`.
- Print counts by transition and reason. `--json` for machine consumers.

Reconciliation **never runs automatically** — a red doctor always means
*un-acknowledged* loss, which is the actionable signal.

### 3. Doctor capture arm: pruned is terminal, fresh loss still fails

In `check_capture` (`doctor.rs:163`) add a `pruned` arm that increments a counter
and skips the file checks. `complete` keeps its existing missing-file failure
(`doctor.rs:237-276`). Surface the pruned count in the ok/warn detail
(`"{launches} launches, {turns} turns, {pruned} pruned, {bytes} bytes"`) so the
count is visible without inflating failures. Edit is confined to the
capture-specific arm — the `Status`/`Check` shared classification (W2-236's turf)
is not touched. **Coordinate with W2-236 before merge.**

### 4. Unify `trace_root()` with the store home resolver (survive env differences)

Change `trace_root()` to `crate::store::lf_home_dir().join("traces")` so
capture-time and doctor-time roots resolve identically under every build and env
(release default is unchanged: `~/.lf/traces`). This closes the latent
divergence and is a prerequisite for reconciliation to be *safe* — it must not
tombstone a file that a correct resolver would have found. `lf_home_dir()` is
`pub(crate)`, reachable from `trace.rs`. **Already applied in the working patch**
(`trace.rs` uncommitted edit); survives the rebase as authored work.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does any code delete trace files or launch rows? | No. Only insert-rollback `remove_*` (fire on DB failure). No retention/GC/TTL. `delete_wave` has no trace cascade. | The contract is "external removal must be tombstoned by an explicit operator step," not "internal deletion must be atomic." No retention to fix. |
| Can the write path leave `complete` with a missing file? | No. staging→fsync→rename precedes the DB insert; `complete` set only after a second fsync in `finish()`. | Write path needs no change; the 235 are post-completion external loss. |
| Are the 235 really gone, or misrooted? | Genuinely gone — run dirs absent from the release `~/.lf/traces`; doctor ran release/default home. | Tombstone+reconcile is the core fix; trace_root unification is a separate latent-bug fix, not the cause of the 235. |
| Can SQLite widen a CHECK enum in place? | No — CHECK is baked into the table. | Table rebuild migration (precedent: `0.11.002`). Runner disables FK actions + `foreign_key_check` + backs up. |
| Will reconcile hide a real capture bug? | Risk is real if it auto-sweeps recent losses. | Never auto-run; 48h age guard by default; report recent-missing separately; doctor stays red on fresh `complete`-missing. |
| Does trace_root unification break existing refs? | Release+default: identical path (`~/.lf/traces`), no change. Dev builds: path moves to `~/.lf-dev/worktrees/<id>/traces`; dev stores are disposable and per-worktree. | Safe for production; note the dev-only path change in the migration/PR. |
| Does changing `finish()` order or adding states break turns? | Turns cascade off launch via FK; `pruned` is launch-level only; turn statuses unchanged. | No turn-schema change needed. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Auto-tombstone missing files inside `lf doctor` | Zero operator effort | Doctor becomes a mutator; masks fresh loss (the exact anti-goal); steps on W2-236's read-only classification |
| Delete the 235 rows instead of tombstoning | Simplest; doctor goes green | Destroys history the store is meant to keep; loses the token/spend accounting those launches carry; no audit trail |
| Reuse `partial` for gone files | No migration | Conflates "we captured some of it" with "the artifact is gone"; doctor already fails `partial`, so no green path |
| Only fix `trace_root()` divergence | Small | The 235 are genuinely absent under the correct root; unification alone leaves them red |
| Add retention/GC now (delete old traces, tombstone as we go) | Attacks unbounded growth (141 MB) | Scope creep; not the reported bug. The `pruned` state + tombstone-before-delete ordering is the *contract* a future retention task drops into — designed for, not built now |

## Key decisions

- **`pruned` is a first-class terminal state, not a flag on `complete`.** A
  distinct enum value lets doctor branch cleanly and keeps "captured and present"
  vs "captured and gone" honest in one column.
- **Tombstoning is always an explicit operator action.** `lf runs reconcile
  --apply`, never doctor, never a background job. This is what makes a red doctor
  mean "un-acknowledged loss" and keeps fresh loss a failure — the task's central
  requirement.
- **48h age guard is the anti-masking device.** Without it, `--apply` during an
  active capture regression would tombstone the very evidence. Recent missing
  captures are reported, not swept, unless `--all` is passed.
- **Reconciliation lives in `lf runs`, the capture arm in `check_capture`.** This
  respects the W2-235 (capture lifecycle) / W2-236 (shared doctor classification)
  split. The only doctor edit is the capture-specific `pruned` arm; coordinate
  before merge.
- **Prevention contract for any future retention: tombstone-before-delete.** Set
  `pruned` in a committed transaction, *then* unlink the file. A crash between
  leaves a `pruned` row pointing at a still-present file — harmless, since doctor
  treats `pruned` as terminal regardless of the file. Documented in the trace
  module for the future retention task; not implemented here.

## Scope

- **In scope:** `.020` migration adding `pruned` (next free ordinal post-rebase;
  `.018`/`.019` claimed on main by #986 and `task_pr_github_observation`);
  `finish()`/write-path unaffected but reason plumbing for tombstones; `lf runs
  reconcile [--apply] [--all] [--json]`; `check_capture` `pruned` arm + pruned
  count in detail; `trace_root()` unification with `lf_home_dir()` (already
  applied in the working patch); behavior tests; a production-shaped
  doctor+reconcile proof against a **copied** store.
- **Sequencing (directive v3):** #986 merged 2026-07-16, so rebase onto main
  first, allocate `.020` mechanically, then finish reconcile/tombstone impl +
  tests. Keep canonical main clean.
- **Out of scope:** building trace retention/GC (only the state + ordering
  contract it will use); rewriting or deleting the production DB by hand; changes
  to `agent_turns`/context schema; W2-236's shared classification helpers; the
  interactive `prompt_only` warn path.

## Done when

1. `cargo test` passes, including new tests:
   - migration widens the enum and preserves all existing launch rows
     (round-trip fixture under `tests/fixtures/dto/` if the wire shape changes);
   - a `complete` launch with a deleted file → doctor `fail`; after `reconcile
     --apply` → `pruned` and doctor `ok`; a *new* deleted-file `complete` → `fail`
     again (fresh loss survives);
   - reconcile leaves a `<48h` missing capture as a reported candidate, not a
     tombstone, unless `--all`;
   - orphaned `capturing` (dead process) → `partial` with reason;
   - `trace_root()` resolves through `lf_home_dir()` (dev/release/control-home
     cases).
2. Against a **copied** production store (`LF_HOME=/tmp/w2-235`): `lf doctor`
   capture check goes `fail (235)` → `lf runs reconcile --apply` → `ok (235
   pruned)`, and the demo's step 4 fresh-loss failure reproduces.
3. Production DB untouched (the fix ships the *command*; a human runs `lf runs
   reconcile --apply` against real `~/.lf` when they choose).

## Measure

- Baseline: `lf doctor --json` capture check on a copy of prod → `235 failure(s)`.
- After `lf runs reconcile --apply`: `0 failure(s)`, `235 pruned` in the ok detail.
- Regression sentinel: inject one fresh missing `complete` file → capture check
  reports exactly `1 failure(s)`, proving the surface stays sensitive.

## Wave alignment

Serves infrastructure's **Developer Efficiency** KR "Avoidable human-in-the-loop
setup or repair steps found in agent runs fall to zero": a permanently-red Home
health surface is a standing repair tax that hides real signal. This turns 235
un-actionable failures into an explicit, one-command acknowledgment while keeping
the surface honest about fresh loss. New risk introduced — reconcile could mask a
live capture bug — is bounded by the 48h age guard and the never-auto rule; noted
here per the wave-memory discipline.
