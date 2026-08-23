# Complete pre-land evidence on every landing

## Problem

Loopflow already writes schema-3 pre-land records under the Git common
directory, but the record is not yet a complete receipt. A normal phase failure
is checkpointed, while `KeyboardInterrupt` or `SIGTERM` escapes `run_plans`
before `finished_at`, the terminal status, postflight build bytes, and the
active phase's duration/CPU are written. `load_gates` then excludes the record
because it has no terminal time. Resource collection also represents both an
unsupported measurement and a measured zero as `None`/fallback values, so the
scorecard cannot distinguish a real boundary from dropped capture.

The second break is attribution. The Task PR merge request is the durable
settlement intent written before GitHub is armed; `pr_landings` supervises that
intent only after arming. Neither carries a pre-land receipt identity. The
scorecard therefore counts free-floating gate runs rather than proving which
local gate belonged to a landing. Repeated ad hoc runs can inflate the
population, while a landing with no local evidence is invisible.

This prevents the Performance & Efficiency project from starting either its
thirty-consecutive-landing stage streak or its resource-evidence window. The
current 14-day evidence makes the failure visible: changed gates are 29/46,
full gates 7/13, rust 45/48, swift-boundaries 30/32, and build disk plus child
CPU only 3/59.

## The demo

Run a deterministic fixture containing a passing gate, a phase failure, and a
caught interruption, then render the scorecard. Every required stage shows an
explicit status and duration, supported build-disk and child-CPU rows show
`3/3`, and the synthetic Task PR merge request resolves to exactly one immutable
receipt.

## Approach

Evolve the existing Git-common JSON record into schema 4 and keep it as the
single authority for local gate facts:

```text
<git-common-dir>/loopflow/pre-land/runs/<kind>/<receipt-id>.json
```

`GateReceipt` has one stage list. That list is both the selected plan and its
result, so plan and execution cannot drift into parallel arrays. Each entry
names `suite`, `stage`, `budget_seconds`, `status`, `executed`, and
`duration_seconds`. Selected work begins as `not_run`, `executed=false`, and an
explicit unsupported duration whose reason is `not_run`. A started stage always
finishes with a measured partial or complete duration, including a real zero or
interruption. The top-level
`plan_fingerprint` continues to identify the exact commands without persisting
commands, output, prompts, environment, or diffs.

The terminal shape is:

```json
{
  "schema": 4,
  "receipt_id": "20260823T120000Z-123-abcd1234",
  "kind": "changed",
  "branch": "jack-heart/capture-complete-pre-land-stage",
  "head": "...",
  "worktree": "...",
  "tree_fingerprint": "...",
  "plan_fingerprint": "...",
  "started_at": "2026-08-23T12:00:00Z",
  "finished_at": "2026-08-23T12:04:00Z",
  "status": "interrupted",
  "failure_kind": "signal",
  "stages": [
    {
      "suite": "rust",
      "stage": "clippy",
      "budget_seconds": 900,
      "status": "interrupted",
      "executed": true,
      "duration_seconds": {"state": "measured", "value": 37.2}
    },
    {
      "suite": "rust",
      "stage": "rust",
      "budget_seconds": 1200,
      "status": "not_run",
      "executed": false,
      "duration_seconds": {"state": "unsupported", "reason": "not_run"}
    }
  ],
  "resources": {
    "build_disk_bytes": {"state": "measured", "value": 0},
    "child_cpu_seconds": {"state": "measured", "value": 12.4}
  }
}
```

Every evidence-bearing scalar uses one tagged shape:

- `{"state":"measured","value":0}` is a real zero.
- `{"state":"unsupported","reason":"postflight_resource_snapshot_failed"}`
  names the boundary that prevented capture.

There are no nullable schema-4 stage/resource measurements. Diagnostic resource
context may remain, but `build_disk_bytes` is the final current-worktree build
measurement and `child_cpu_seconds` is one gate-level `RUSAGE_CHILDREN` delta.
The gate-level value replaces the current sum of optional phase values, which
loses the interrupted phase and excludes the resource-check children.

`_GateRecorder.finish()` owns the only terminal transition. `run_plans` creates
the running receipt from the selected plan before resource preflight, starts one
child-CPU baseline at that boundary, and wraps preflight, suites, and postflight
in one terminalization boundary. A preflight block is therefore a real terminal
receipt rather than a late reconstruction. Temporary main-thread handlers turn
`SIGINT` and `SIGTERM` into a typed `_GateInterrupted` exception and are restored
on exit; the signal handler itself performs no file or subprocess work.
`_run_command`, while it still owns the child and pump thread, catches the typed
exception, stops and reaps the process group, and attaches the active stage's
partial duration. `run_plans` then leaves later selected stages explicitly
`not_run`, attempts postflight resource measurement, finishes the receipt, and
returns the conventional signal exit code. Ordinary phase failures, resource
blocks, suite pre/postcheck failures, and unexpected runner exceptions use the
same finish path. An unexpected exception is re-raised only after the failed
receipt is durable.

The writer keeps the current temp-file + `os.replace` protocol, adds an fsync of
the parent directory after the rename, and refuses a second terminal write. A
schema-4 checkpoint write failure aborts the gate before more work can run; a
terminal write failure returns a nonzero capture error and preserves the latest
running checkpoint when the filesystem still permits it. Capture never degrades
to a warning that silently disables the recorder. Running checkpoints remain
useful crash evidence. The scorecard never converts
an uncatchable crash or power loss into a fabricated terminal time: it surfaces
the nonterminal receipt as `UNKNOWN: missing terminal boundary` outside the
windowed percentiles. This is the one genuinely unsupported interruption
boundary; caught operator and supervisor interrupts are complete receipts.

Schema 1-3 files remain readable historical evidence. They are neither
backfilled nor deleted, but only schema 4 can satisfy the new completeness and
landing-link coverage. A scorecard window spanning the cutover stays `UNKNOWN`
until the old evidence ages out.

The runner also owns one non-evidentiary handoff pointer under the same Git
common root, keyed by worktree identity. Starting a real gate invalidates the
old pointer before executing anything. Only a durably passed schema-4 receipt
atomically installs a new pointer containing its relative path; failure,
interruption, or capture error leaves no handoff. Reusing an identical passing
receipt installs that exact reference again. The pointer never copies plan,
stage, resource, or timing facts, and a receipt remains authoritative without
it.

For attribution, add one schema-4 receipt reference to the Task PR's settlement
episode. It is persisted on the Task PR row, set in the same transaction as the
first exact-head merge request, and becomes immutable once that landing merges.
Keeping it beside rather than inside the transient `PrMergeRequest` lets a
landing-owned CI repair revoke and re-arm auto-merge without losing the original
local proof. The migration adds:

- `preland_receipt_authority(singleton, started_at)`, the cutover after which a
  supported repository's landing absence has meaning;
- `task_prs.preland_receipt_ref TEXT`;
- a partial unique index on `preland_receipt_ref`, preventing one receipt from
  satisfying two Task PRs; and
- an update trigger preserving a merged Task PR's receipt reference.

The repository declares structured pre-land support with
`"preland_receipt_schema": 4` in `performance/budgets.json`. For a supported
repository, an initial managed Task `arm` or `land` resolves the exact current
schema-4 handoff for the current worktree before its first local head mutation.
The resolver is owned by `scripts/test.py`; schema 4 replaces the current
all-files tree hash with one shared product-tree fingerprint that excludes
`scratch/**`, while retaining the exact plan fingerprint. The gate review and
PR-copy artifacts therefore cannot invalidate the tested product tree. The
resolver loads the exact referenced receipt rather than scanning by mtime, then
validates schema, terminal pass, worktree, product-tree fingerprint, the
receipt's exact plan fingerprint, and coverage of every stage in the current
required changed plan. A full receipt may therefore satisfy a changed gate, but
an unrelated partial plan cannot. The resolver emits only the relative receipt
reference.

`land_repo`/`arm_current` capture that validated reference once and carry it
through the existing synchronous rebase-recovery retry. An already-armed exact
Task PR reuses its stored reference; a process restart after integration cannot
infer equivalence and requires a fresh gate if the handoff no longer validates.
The final operation passes the captured reference to
`request_task_pr_merge`; the store writes the exact-head merge request and its
evidence reference in the same transaction before `finalize_remote` can arm
GitHub. Once that transaction commits, landing clears the handoff pointer.
`pr_landings` reads the Task PR during supervision but does not copy the
receipt. A crash after arming therefore leaves a durable linked settlement
request for reconciliation rather than an unlinked merge. A stale, red,
interrupted, already-consumed, or content/plan-mismatched receipt is not
eligible and produces the exact rerun command. Budget values never decide
whether a receipt is eligible.

The reference follows the settlement lifecycle rather than the mutable head.
An explicit user/agent head mutation that revokes merge intent also clears the
unmerged reference, so the next initial request must resolve a fresh receipt. A
landing-owned CI repair and re-arm preserves the landing's original local
pre-land reference while its hosted CI incident owns the repaired head. Once
the Task PR merges, the reference cannot change.

Repositories that do not declare this structured protocol and direct non-Task
landings are explicit unsupported populations outside this repository-owned
Project population. Managed Task `submit` is already rejected. Existing
historical Task PRs remain outside the post-cutover eligible population rather
than receiving invented links.

Scorecard coverage then separates evidence completeness from latency samples:

- A required stage is eligible once it appears in a terminal schema-4 plan.
  Its explicit status and tagged duration make its evidence complete even when
  the duration is unsupported because the stage is `not_run`.
- Stage p50/p95 use only `executed=true`, `state=measured` durations. An
  observed `not_run` proves coverage but never makes the stage look artificially
  fast.
- A resource is eligible once the terminal receipt names it. Only
  `state=measured` contributes a numeric sample; `unsupported` stays missing
  with its reason and never becomes zero.
- A landing is eligible once a supported repository's Task PR receives an auto
  merge request after `preland_receipt_authority.started_at` and later merges,
  windowed by GitHub's first accepted `merged_at`. The cutover and managed Task
  settlement boundary define eligibility, so a missing nullable reference is a
  scorecard miss rather than an opt-out. The landing is measured only when its
  unique reference resolves to a passed terminal schema-4 receipt.

`measured_row` therefore accepts a coverage count separately from numeric
samples and emits `sampled` alongside `eligible` and `measured`. This preserves
honest percentiles while allowing a failed/interrupted plan to have complete
stage coverage.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Where does evidence disappear? | `KeyboardInterrupt` is re-raised from `_run_command`; `run_plans` has no outer terminal `finally`. The last file stays `running` with `finished_at=null`, and `gate_in_window` excludes it. | One recorder-owned terminalization boundary handles pass, fail, resource block, caught interruption, and runner error. |
| Is the current write atomic and durable? | The file is flushed and fsynced before `os.replace`, but the containing directory is not fsynced. | Keep the existing compact file authority and fsync the directory after the terminal rename. |
| Can current resource nulls distinguish zero from unsupported? | No. `_resource_receipt` returns `None`, falls back from a missing postflight to preflight, and sums only completed phase CPU values. | Tagged measurements replace null/fallback semantics; final build bytes require postflight, and child CPU is one gate-level delta. |
| Why do failed plans lower phase coverage? | `phase_rows` counts every planned phase as eligible but drops `not_run` from values, and `measured_row` equates numeric samples with evidence coverage. | Count explicit status plus tagged duration as complete stage evidence; calculate latency only from measured executed samples and publish `sampled`. |
| Can land identify the reviewed gate after it stages and clears `scratch/` or retries a rebase? | The current tree hash includes every scratch artifact, while `prepare_pr` commits and then deletes those handoff files before final integration. A scan for the newest run could also select an unrelated manual gate. | Schema 4 shares a scratch-excluding product-tree fingerprint, and the runner atomically names the exact passed receipt in a pointer. Land validates and captures that pointer before head mutation, then carries it through its synchronous retry. |
| What owns the landing population? | `request_task_pr_merge` persists exact-head settlement intent before GitHub is armed; `pr_landings` is created later to supervise it. | Set the receipt reference on the Task PR in the same transaction as its merge request so a crash after arming cannot create an unlinked merge. The watcher reads rather than copies it. |
| Can the reference be persisted after arming? | No. `watch_armed_pr` starts only after `finalize_remote`; GitHub could merge or the process could die first. | Persist the reference atomically with the Task PR merge request before the remote side effect. |
| What happens when CI repair advances the head? | The repaired head is owned by the same watched landing and its typed CI incident; treating re-arm as a second local landing would either duplicate evidence or block repair. | Preserve the initial local receipt during landing-owned re-arm. Clear it only when merge intent is explicitly revoked outside that landing; a later initial request must resolve a fresh receipt. |
| Can one old pass satisfy many landings? | The current reusable-run scan has no consumption concept. | A partial unique index makes the receipt-to-landing relation one-to-one; joining the same durable landing remains idempotent. |
| Should a budget regression block shipping? | Wave doctrine says budgets judge evidence, not correctness. | Eligibility checks receipt identity, terminality, pass status, and content/plan match only. Numeric budgets remain scorecard judgments. |
| Can cleanup create compliance by deleting evidence or work? | Resource recovery currently removes allowlisted inactive build roots and old `.lf/tmp/gate` artifacts, while durable receipts live under the Git common directory. | Preserve that boundary and add a regression proof that source, worktrees, SQLite, and receipt roots are never recovery targets. |
| Does a hard kill have an authoritative finish time? | No owner remains to report it; using the next scorecard read or file mtime would invent the terminal boundary. | Surface the nonterminal checkpoint as named `UNKNOWN`; do not place it in a time window. Catch and finish SIGINT/SIGTERM, the supported interruption paths. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Move all gate evidence into SQLite | Foreign keys would make landing linkage easy, but the repository test runner would need Home database authority and migration compatibility while it is testing candidate source. Failed standalone gates would also become coupled to Loopflow runtime availability. | It replaces a working repo-owned durability boundary and risks a second writer/API solely for telemetry. |
| Reconstruct stages and resources from logs, traces, process tables, or artifact mtimes | Avoids changing the writer. | Observer timestamps are not terminal facts, logs may be pruned, commands do not prove execution, and missing values become guesses. This is the failure class the task exists to remove. |
| Write one append-only file per stage and aggregate later | A hard kill would leave more fragments. | It creates a second aggregation protocol, cannot atomically state the complete plan, and makes landing linkage one-to-many. |
| Let land select the newest passing receipt | Avoids a handoff pointer. | Timestamp order is not attribution: another manual gate in the same worktree can be newer, and integration changes the current HEAD. The exact pointer contains no copied evidence and fails closed when stale. |
| Store a copy of the full receipt on `task_prs` or `pr_landings` | A landing query becomes self-contained. | The Git-common receipt and SQLite copy could diverge. Settlement needs one immutable reference, not another telemetry authority. |

## Key decisions

The wild-success version is boring: Gate writes one terminal fact regardless of
outcome, land points at it, and telemetry only counts what owners explicitly
reported. A maintainer can inspect a `3/3` row and know it includes red and
interrupted attempts without those attempts distorting latency percentiles.

The wild-failure version is a compatibility thicket: schema-3 nulls get
silently coerced, stage and plan arrays drift, SQLite acquires copied resource
columns, and land selects the newest file by timestamp even when it belongs to
another tree. The design forbids each of those shortcuts.

Receipt capture and landing linkage are one coherent change. Shipping only the
writer would improve free-floating gate statistics without starting the
landing KR. Shipping only the link would make incomplete receipts look
authoritative. Schema 4, terminalization, the unique link, and scorecard
semantics land together.

## Scope

- In scope: schema-4 gate receipt types and validation in `scripts/test.py`;
  SIGINT/SIGTERM and exception terminalization; final build-disk and gate-level
  child-CPU capture; explicit unsupported reasons; atomic file durability;
  exact receipt handoff and resolution for supported repositories; durable
  linkage on managed Task PR settlement requests made by `lf pr arm`/`land`; scorecard
  coverage/sample separation; deterministic
  Python and release-equivalent Rust fixtures; user documentation.
- Out of scope: hosted CI job telemetry; treating every `lf pr arm` request as
  a watched `pr_landing`; treating `lf pr submit` as an eligible managed Task
  landing; inventing a generic test-plan protocol for repositories that do not
  declare structured support; backfilling schema 1-3 records or historical
  landings; changing build/CPU budgets; deleting or relocating source,
  worktrees, durable state, receipts, traces, or caches outside the existing
  recovery allowlist.

## Done when

- `uv run pytest python/tests/test_gate_bounded.py python/tests/test_lifecycle_scorecard.py -q`
  passes a mixed pass/fail/SIGINT fixture and asserts complete supported-stage,
  build-disk, and child-CPU coverage without counting `not_run` as a latency
  sample.
- The same fixture asserts explicit measured zero and explicit unsupported
  reasons survive JSON round trips.
- Handoff fixtures prove gate start invalidates a stale pointer; only pass or
  exact-tree reuse installs one; scratch-only changes preserve validity; a
  product or required-plan change, failed/interrupted run, and consumed receipt
  all refuse with the focused rerun command.
- A release-equivalent Rust store fixture applies the materialized migration,
  creates Task PR merge requests with receipt references, counts a post-cutover
  request without one as missing coverage, and rejects reuse of one reference
  by another Task PR.
- Repeating the same exact-head merge request preserves its first request time
  and receipt reference; explicit head mutation clears the unmerged reference
  and requires a fresh eligible receipt, while landing-owned CI re-arm preserves
  the original reference. A synchronous rebase-conflict retry carries the same
  validated reference; a restarted, no-longer-matching integration does not
  infer equivalence.
- A scorecard fixture proves historical/unlinked landing rows remain named
  unknown, a schema-4 linked merge is measured, and a nonterminal crash
  checkpoint is surfaced without an invented `finished_at`.
- Resource recovery tests prove that source files, active worktrees, the Home
  database, and `<git-common-dir>/loopflow/pre-land/runs` are unchanged.
- The existing focused baseline remains green; at kickoff it was `45 passed`.

## Forbidden outcomes

- No stage/resource facts copied into a new SQLite telemetry table, Task PR
  columns, or landing JSON blob.
- No newest-file or timestamp heuristic chooses a landing receipt; the handoff
  is an exact reference and never becomes a second evidence record.
- No `null`, omitted field, fallback-to-preflight value, or inferred zero for a
  schema-4 measurement.
- No scorecard or recovery process fabricates `finished_at` from observation
  time, file mtime, logs, or process absence.
- No `not_run` duration enters latency percentiles.
- No receipt can satisfy two Task PRs, and no merged Task PR can replace its
  receipt reference.
- No old-schema backfill, compatibility write path, dual write, or deletion to
  make coverage green.
- No raw command, output, prompt, environment, diff, task id, or secret enters
  the receipt.
- No budget verdict changes the gate's product result or the landing's merge
  correctness decision.

## Internal slices

1. Replace schema-3 null/fallback semantics with the schema-4 receipt and one
   terminal recorder path. Add caught-interruption and resource-support tests.
2. Add the exact passed-receipt handoff and supported-repository resolver, then
   carry its immutable reference through Task PR merge request settlement and
   release-equivalent store proofs.
3. Join landing eligibility to receipt authority in the scorecard, separate
   coverage from numeric samples, and add the mixed deterministic fixture.
4. Update `TESTING.md`, `performance/README.md`, and the gate handoff guidance;
   run the focused proof once for the final tree.

## This slice

Land all four internal cuts in this PR. The schema and link are indivisible:
there must be no interval where new receipts cannot be scored or new eligible
landings can be written without their unique receipt.

## Slice ledger

- 2026-08-23 kickoff: confirmed the current escape path, nullable/fallback
  resource semantics, phase coverage conflation, and missing landing link by
  reading `scripts/test.py`, `scripts/lifecycle_scorecard.py`, `pr_landings`,
  and their fixtures.
- 2026-08-23 baseline: `uv run pytest python/tests/test_gate_bounded.py
  python/tests/test_lifecycle_scorecard.py -q` -> 45 passed in 6.95s.
- 2026-08-23 design review: replaced the inferred `not_run=0s` value with
  tagged unsupported evidence, deleted a redundant tracking boolean, pinned
  landing attribution to an exact handoff instead of newest-file selection,
  and kept the Task PR settlement episode authoritative across CI repair.

## Measure

Add `preland_landing_receipt_coverage` as the direct Project signal.

- Owner: Performance & Efficiency, produced by `telemetry-daily` through the
  lifecycle scorecard.
- Eligible population: managed Task PR auto-merge requests created by `lf pr
  arm` or `lf pr land` for repositories declaring schema-4 support, terminally
  merged after the linkage cutover.
- Measured: the landing has exactly one resolvable, passed, terminal schema-4
  receipt and every required stage has status/duration evidence. Resource rows
  separately require `state=measured`; an unsupported field remains explicit
  missingness.
- Target: 100% coverage and a streak of 30 consecutive eligible merged
  landings. Build-disk and child-CPU coverage must remain 100% for 30 days
  before the resource KR can hold.
- Missed response: reset the consecutive streak, report `FAIL` with the first
  missing/unsupported boundary, and open focused capture work; do not reinterpret
  the gate result or delete evidence.
- Met response: increment the streak and preserve the dated scorecard evidence
  used to judge the KR.
