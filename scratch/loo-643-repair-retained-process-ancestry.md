# LOO-643: Repair retained Process ancestry without losing Session history

Status: slices 1–5 implemented and synced with main; gate passed (2026-10-27); PR and land remain. Jack Heart authorized an end-to-end
autonomous fix, local hotfix and patch release. No ad-hoc SQL writes to the live store.

## Problem

`lf self doctor` fails `lineage` on Jack's Home: ended AgentProcess
`275f50e4-…` (Codex, Session `5cbf9613…` "MTG-159 Training scale ladder") names
parent and attached LfProcess `4aaad068-…`, which has no row. Operators need a
supported repair that keeps the Session readable and resumable, records what changed,
and stops the defect from recurring. Hiding the check or deleting history is not success.

## Cause (established, 2026-10-27)

Reproduced on an `lf clone create` snapshot of the live Home (doctor:
`FAIL lineage 1 parent process id(s) are missing`).

- **Not pruning, not historical trace attribution.** `attach_in`
  (`store/sqlite/processes.rs`) reserves the agent row with
  `trace_id = COALESCE((SELECT trace_id FROM processes WHERE id=parent), own id)`.
  The row's trace equals its own id, so the parent had **no row at reservation**.
  Live store: 1 of 7,233 agent rows is self-rooted, and that one is the dangling row.
  All 7,233 agent rows name a parent. Retention (and #1718's `FORGETTABLE`) never
  forgets an lf row that has an agent child or is an agent's attached LfProcess.
- **Partial capture write.** `journal::ledger_insert` is best-effort. At the time,
  0.14.16's slow usage deletes held the write lock (see the cbda52 analysis).
  `lf.log` shows 22 `ledger insert failed … database is locked` (15 s wait) entries
  at 05:25–05:27 UTC. The reservation was at 05:26:25 UTC. The lf process kept its
  in-memory id, then `claim_session_attachment` → `attach_in` succeeded in its own
  Immediate transaction under that unrecorded id. The exit/interrupt write also
  never landed. The Session's admitting `lf session connect` (`ad30403c`) has a row.
  Which lf process owned `4aaad068` is unrecorded, and no live process carries that id.
- The journal already prevents this for lf children: `parent_is_recorded` drops an
  unrecorded parent ("a legitimate parent records its own start row before it can
  spawn anything"). Agent reservation had no equivalent.

**Resumability is broken today, not only doctor.** `with_session_resume` refuses
unless the attached LfProcess is proven Dead. `process_evidence` returns Unknown for
a missing row, so `lf session connect 5cbf…` refuses with "Conversation already has
an attached LfProcess". (The clone blocks provider continuation by design, so the
proof is a store test; see Done when.) Resume would not self-heal doctor either: the
old agent row stays unreferenced history, and agent rows are never forgotten.

## Demo

```
lf monitor prune --dry-run   # agent 275f50e4: would_repair — parent LfProcess 4aaad068 was never recorded
lf monitor prune             # repaired 1; Session 5cbf… records the change
lf monitor prune             # nothing to repair
lf self doctor               # ok lineage
lf history show 5cbf961350cd4c98b7fccc4a491c502d   # observed: agent_process_ancestry_repaired {…4aaad068…}
```

The repaired Session then resumes through ordinary `lf session connect`.

## Approach

### Prevention: the attaching lf's row lands with its attachment

Every attachment write records the attaching LfProcess's start row in the same
Immediate transaction, `INSERT … ON CONFLICT(id) DO NOTHING`. The fields are the
ones `ledger_insert` writes at start: id, trace, parent, pid, os_started_at, command,
repo, cwd, started_at, via_agent and caller fields. An attachment can no longer
commit under an unrecorded parent. A later completion upsert from `ledger_insert`
completes the same row. If that write also fails, `close_dead_processes` closes it
by pid evidence.

- `journal::current_start_record(process)` returns the current Process's start row,
  built by the same `start_record` that `ledger_insert` uses, so the two cannot drift.
- Attachment APIs keep `&LfProcessId` (decision recorded in questions.md): every
  production caller attaches as `journal::current_lf_process_id()`, and `attach_in`
  writes that start row (`record_process_in`) when the attacher is the current Process.
  Fixtures attaching as another id record it first (`SqliteStore::test_lf_process`).
- `attach_in` takes the agent's trace from the parent row directly. **Delete** the
  `COALESCE(…, own id)` fallback. A missing parent is now impossible, and the NOT
  NULL constraint turns it into an error, not a self-rooted row.

### Repair: `lf monitor prune` releases ancestry it can prove was never recorded

`lf monitor prune` already owns AgentProcess cleanup (`reap_in`, with per-record
action/reason). Add one pass, `repair_unrecorded_ancestry(store, dry_run)` in
`harness/agent_process.rs`, reported in `ProcessPruneReport.agent_process_records`
with action `would_repair`/`repaired`. It runs only from `run_prune`, not from lazy
`recovery::recover()`.

Selection is exactly the proven signature, all required:
- `kind='agent'`, `completed_at IS NOT NULL` (the provider exited; busy Sessions are untouched)
- `parent_lf_process_id` has no row, and `trace_id = id` (absent at reservation, not pruned later)
- `attached_lf_process_id` is NULL or has no row (no recorded owner could still hold it)

One Immediate transaction per row:
1. Upsert the repairing `lf monitor prune` Process's own start row (prevention helper).
2. Insert a Session fact: `session_events` kind `observed`, `lf_process_id` = the
   repairing Process, payload `{type:"agent_process_ancestry_repaired",
   agent_process_id, unrecorded_parent_lf_process_id,
   released_attached_lf_process_id, evidence:"trace_rooted_at_agent"}`.
3. `UPDATE processes SET parent_lf_process_id=NULL, attached_lf_process_id=NULL,
   attachment_token=NULL` guarded by the same predicate. Zero rows changed rolls
   back, so a second run (or a racing one) is a no-op.

After the repair, the agent row is an honest trace root with no recorded parent.
The Session fact keeps the id it named and why it was removed. `attachment_in`
returns `None` (no token) before it reads the parent, so `with_session_resume`
skips the dead-owner proof. The ended agent needs no settlement, and `attach_in`
reserves a fresh agent under the resuming lf. A stale holder of the old token fails
its fence. Native history (`provider_thread`), the Task binding and Session events
stay unchanged.

### Doctor

`check_lineage` keeps failing on any dangling or cross-trace parent. When a dangling
child matches the repair signature, the detail names the remedy ("N AgentProcess
parent(s) were never recorded; `lf monitor prune` repairs them"). Other dangling
parents keep the generic failure. No allowlist and no acknowledgement table.

Concept review (2026-10-27) found the hint re-derived the signature in Rust and
omitted the attached-owner clause. Resolved: doctor's `run` counts
`store.unrecorded_agent_ancestry()` (the repair's own `UNRECORDED_ANCESTRY`), and
`audit`/`check_lineage` take that count. A recorded-owner fixture fails lineage
without the hint and prune leaves it alone.

Second concept review (2026-10-27, after main sync): the model, names and action
vocabulary (`would_repair`/`repaired`/`retained`, matching reap's) hold. One gap:
the doctor hint sends operators to `lf monitor prune`, but its help ("Reap
registered orphan providers and remove dead process receipts") and
`docs/troubleshooting.md` "Orphaned agent processes" describe only reaping. Next
implement: the prune `about` names the repair, regenerate `docs/lf-reference.md`,
and the troubleshooting section adds `lf self doctor` failing `lineage` → prune
releases ancestry that was never recorded, keeping a Session fact.

Third concept review (2026-10-27, through `65ed22f566`): slice 5 closed that gap;
the help, troubleshooting and doctor hint now tell one story. The `start_record`
cwd compression changes no concept. Nothing to change and no open product choice.

## Alternatives rejected

- **Backfill a placeholder parent row.** Its started_at, trace and command are
  unknown. Any value is fabricated, and a completed row with no command fails
  `attribution`.
- **Null only the parent.** `attachment_in` then errors "AgentProcess has no parent
  LfProcess", which makes the Session unreadable for attachment and still not resumable.
- **Doctor exemption for the signature.** That hides the warning and leaves the
  Session unresumable.
- **Refuse attachment when the start row is missing.** A guard that turns lock
  contention into failed Session launches. Writing the row in the attachment
  transaction removes the failure class instead.
- **Repair inside `retention::prune` or lazy `recover()`.** That collides with
  #1718's retention rewrite and adds a full agent scan to every read.

## Contract

- **Source of truth:** `processes` (agent row parent/attachment) and
  `session_events` (repair fact). Doctor, `lf history show`, Session attachment
  and `lf monitor prune` read these.
- **Absent/error states:** a live agent, a recorded parent, or a trace ≠ id is
  never repaired. Doctor reports it. An unknown attached owner with a row is left
  to the existing resume/takeover rules.
- **Exclusions:** the store lock contention itself (db678e, LOO-632 reporting),
  stale binaries (LOO-634), and #1718's exit-time forgetting.
- **No schema change, no migration draft.**
- **Forbidden outcomes:** placeholder parent rows; a doctor allowlist; a second
  "unrecorded" representation alongside NULL parent plus Session fact; repair from
  lazy reads; any edit to `retention.rs` here.

## Coordination

#1718 (LOO-615) merged to main on 2026-10-27 (`8bf860806c`) and is merged into this
branch: `record_process` calls `record_process_in` and then forgets per `FORGETTABLE`;
doctor's `audit` is gone (main inlined it), so the repair test calls `check_lineage`.
Its predicate already retains what this change relies on (kickoff, 2026-10-27):
the prevention row is named by its agent child (`parent_lf_process_id`) and as
`attached_lf_process_id`, and the repairing prune Process is named by the repair's
`session_events` fact. No `FORGETTABLE` edit is needed.

## Slices (one PR)

1. Done: prevention (start record in the attachment transaction, COALESCE deleted).
2. Done: `repair_unrecorded_ancestry` in `lf monitor prune` (`UNRECORDED_ANCESTRY`
   predicate in processes.rs).
3. Done: doctor's remedy count reads `unrecorded_agent_ancestry`.
4. Done: sync with main (#1718).
5. Done: prune help and troubleshooting name the ancestry repair (concept review).
6. Done: gate (2026-10-27).
7. Remaining: PR, land. The root operator owns the shared patch release,
   `lf install`, and the live `lf monitor prune` readback after merge (Jack's steer, 2026-10-27).

## Done when

- Store test: a fixture of the live shape (ended self-rooted agent, missing
  parent/attached, `agent_sessions` pointing at it) refuses
  `resume_session_attachment` before repair. After repair it resumes with a new
  agent row, the Session's prior events and `provider_thread` are unchanged, and
  the repair fact names `4aaad068`-equivalent ids.
- Second repair run changes nothing and reports nothing to repair.
- Doctor: the repaired fixture passes `lineage`. A pruned-parent fixture (trace ≠ id)
  and a live-agent fixture still FAIL and are untouched by `lf monitor prune`.
- Prevention test from slice 1 passes. `grep -n "COALESCE((SELECT trace_id"
  rust/loopflow/src/store/sqlite/processes.rs` returns nothing.
- Gate: `cargo test -p loopflow --lib agent_process processes doctor` plus the
  integration suites touching attachment
  (`session_lifecycle_tests`, `process_ownership_tests`), `cargo clippy --all-targets -- -D warnings`.
- Live, after the release is installed: `lf monitor prune --dry-run` lists exactly
  `275f50e4…`; `lf monitor prune` repairs it; a rerun is a no-op; `lf self doctor`
  shows `ok lineage`; `lf history show 5cbf961350cd4c98b7fccc4a491c502d` shows the
  fact; the Session's attachment reads as released (no attached LfProcess) in
  `lf session show`/status JSON. `--prepare` only reads the transport owner and
  never reaches the resume claim, so it proves nothing here. Resuming Jack's MTG-159
  conversation is his call; the store test carries the resumability proof.

Gate (2026-10-27, through `78a68913cc`, host load ~150–310): `cargo fmt --all -- --check` ok; `cargo clippy --all-targets -- -D warnings` clean; `cargo test -p loopflow --lib` 2150 passed before the run hit its own 90-minute tool deadline (SIGTERM), and the 113 failed/unfinished tests rerun with `--exact` → 103 passed, 10 ignored, 0 failed (the 78 earlier failures, in untouched chapter/store/status/graph tests, left no panic output); `cargo test -p loopflow --test process_ownership_tests --test session_lifecycle_tests` → 12 + 29 passed. Full matrix stays with CI.

LOO-647 (in flight) moves `retention::prune` into `top::run_prune`/`ProcessPruneReport`; Jack Heart sent its worker this repair's `would_repair`/`repaired`/`retained` contract so one prune path keeps both. Root owns the installed dry-run/repair/no-op/history acceptance.

Kickoff recheck (2026-10-27, origin/main `e970aec679`, 11 commits ahead): the plan
holds. #1729 (LOO-634) adds `lf monitor rollover` beside prune; the only merge
conflict is the `MonitorCommand` enum in `monitor.rs` (keep both Rollover and the
new Prune help), so the sync must regenerate `docs/lf-reference.md`. Rollover resumes
through the ordinary Session path and attaches no Process itself, so prevention covers it.
v0.14.19 (#1735) shipped without this change; the patch release follows landing.
