# Live Session connection performance

Accepted direction from Jack Heart, October 4, 2026.

Jack Heart wants lf session connect to open an already-live Session quickly, without waiting through unnecessary discovery or reconnect work. Optimize the real connection path until the retained conversation is visible and usable. Jack requested autonomous performance investigation, implementation and management on October 4, and explicitly selected live Session connection rather than resuming a stopped Session.

Acceptance:

* Build or extend an unattended repeatable runner for the production lf session connect path. Measure invocation-to-visible-output and invocation-to-usable-input separately, using owned test Sessions and representative retained history. Exercise repeated connects and the supported provider paths; record unsupported paths explicitly.
* Capture a dated baseline, identify contributing reads/process launches/transport work, choose numeric median/p95 targets from that baseline before optimizing, and compare repeated samples on the same host and data. Retain sample counts, failures/timeouts and observer limitations; a query microbenchmark alone does not prove connection usability.
* Preserve the existing live provider, Session identity, history, drafts and applicable terminal ownership. No duplicate provider, automatic replacement, interruption of unrelated live Sessions or writes to the installed Home as a hidden benchmark prerequisite. Use isolated fixtures and test-owned live processes for active probes.
* Fix measured bottlenecks, rerun autonomously against the recorded targets, and retain regression coverage and a concise before/after report. No manual clicking or human performance demo is required. Do not treat missing evidence or a quiet process as proof of usability.

This owns live Session connection, not cold Desktop bootstrap (LOO-376), Task/deep-link routing (LOO-371), or the Task-conversation/Flow model redesign (LOO-353). Coordinate overlapping connection and discovery code and avoid concurrent heavy benchmark sweeps; preserve active work. A stopped Session resume is outside this Task. Honor repository capacity constraints without deleting other work or changing machine-wide installation/authentication.

The design and baseline precede production edits. Jack Heart's October 4
comment `273a14e4-9ba4-4345-8408-7b777c37d182` supersedes the initial publication
restriction: complete implementation, verification, publication and landing with
direct `lf` commands in this checkout. Direct pursue Flow
`bde591b2-bdfd-4181-8e43-1d6dcbc1eadf` owns this work; preserve the idle managed
Flow without restarting it. Product owns direct landing and authored review
boundaries. This authorization does not establish completed verification or delivery.

<!-- loopflow-task-start:ed73e6ad58500f1f690a1c45ff358f5d916c72f4e435c109abae9be4c6a8f84b -->

Jack supplied this reproduction: lf session connect task_7c24c806bfaa464a877352b568384fff:6430c57b-02e4-4ca9-9021-afb5e68c9003:feature:review_kickoff:0. Recorded connect Exec 360dfe3c-8962-47cf-aab8-c4936ddde21a began October 4 at 10:46:14 America/Los_Angeles (1791135974). Jack observed many seconds of delay. The command remains attached; the inspected command record has no separate connected/usable timestamp, so exact latency is unknown. Include this Flow-review Session selector path in reproduction, even if it resolves or launches rather than attaching to an existing provider. Preserve the live Session; reproduce with isolated equivalents.

Additional acceptance: retain connect phase timings keyed to the exact invocation and Session, and expose them through a supported diagnostic command/log view so Product can answer how long a specific connect took afterward. Separate lookup, preparation/connection, first output and verified input readiness from total attached lifetime; record unavailable readiness evidence explicitly.

## Investigation and draft measurement design — October 4, 2026

The earlier disposable-provider authentication blocker was resolved by one dated probe. No
runner, baseline, numeric targets, diagnostics or production edits exist yet.
One isolated provider turn proved the recipe; no connection probe has run.

Source findings rechecked at `58d3b5b3e05135a3267d0f5340e435044440bb49` on October 5 UTC:

- `lf/commands/session.rs::run` creates the runtime and journal before opening
  the shared store and dispatching to `ops/human_session.rs::open`. Measure
  those startup costs as well as lookup; a timer inside `open` alone misses them.
- Conversation lookup resolves the retained Session, workspace admission and
  provider reference. Codex's `connect_live_codex` probes the existing socket,
  claims the driver, and calls `CodexConnection::recover_history` before launching
  the native UI. Recovery still awaits every turn page and its recorded history before UI launch.
  Upstream `7a6ae0d70` moved those writes off the reactor and added independently
  timed fenced dispatch; the caller still awaits recovery. This remains a
  candidate cost, not a measured bottleneck or permission to drop history.
- Flow-review lookup follows a separate branch: lookup, launch lock, repeated
  lookup to validate the same position, surface projection, then
  `open_flow_locked` / `resume_native_session` or review launch. Preserve that
  revalidation and locking; the conversation-only Codex path cannot establish
  performance of Jack's supplied selector. `resume_native_session` currently
  stops existing native clients and resumes with no live relay endpoint. That
  route cannot be counted as preserving a live provider or its draft merely
  because the selector resolves. The runner must classify the observed route
  and retain this acceptance gap; reconciling live review connection may require
  a substantial implementation change.

Proposed runner: invoke the production binary through owned PTYs against an
isolated Home/database and owned engines. Cover ordinary conversation and exact
Flow-review selectors with short and paginated histories. Inventory live attachment
support per provider; Claude/OpenCode do not use the Codex socket path. Record
unsupported paths separately. Preserve engine PID, Session/thread identity and
drafts during each connection; clean up only runner-owned processes.

`connect_live_codex` finishes its claimed driver after UI exit, which can close
the engine. For repeated samples, use supported driver transfers or separately
owned live fixtures with equivalent history. Do not silently benchmark stopped
Session resumes. This is a source finding, not provider acceptance.

Use an external monotonic start before spawning lf. Visible output requires
recognizable retained conversation content at the PTY endpoint; input readiness
requires a unique input/response exchange through the owned provider, separately
timestamped. PTY bytes do not prove compositor display. Quiet output, socket
acceptance and attached lifetime cannot substitute for either endpoint. Retain
timeouts, failures, host/binary/data identities and at least 20 comparable samples
per reported p95. Simulated transport cannot certify native provider usability.

For durable diagnostics, prefer the existing file journal and reuse
`journal::process_elapsed()` for entry-point timing, keeping the external PTY
clock for the full invocation endpoint. Upstream `7a6ae0d70` demonstrated missing
Exec ledger rows under SQLite contention; diagnostics must survive that case.
`ops/wt_timing.rs` supplies an existing file-based timing/report precedent, not
connection measurements. Journal schema and supported reader selection remain
design work; no additional lifecycle store is proposed. Record the exact connect Exec, requested selector,
resolved Session and monotonic elapsed phase values, including lookup,
preparation/connection, first output and verified readiness. Flush observations
while attached; total exit duration is a separate event. Mark unobservable
endpoints unavailable with a reason. Select the supported diagnostic reader after
checking existing journal commands. Transport RPC completion alone must not be
labeled usable input.

Delete — do not maintain: no confirmed deletion target before measurement.
Remove only measured redundant work; retain ownership fences, review revalidation,
history and existing failure semantics.

Review finding: moving history recovery off the critical path without proving
history ordering and provider readiness could create a false speedup. Remaining
work is fixture/runner construction, baseline and numeric targets, durable timing
and its supported reader, measured optimization, regression checks and the
before/after report. Baseline must identify the binary revision because the older
source predates the dispatch and receipt changes. No production reduction is
justified yet. Source inspection establishes no speedup and no Desktop KR credit.

### Authentication evidence and remaining fixture work

The October 5 UTC probe returned an exact marker from one isolated batch turn
using an existing managed Codex account. No credential was copied; the provider
could refresh its login and wrote a rollout in the account home. The disposable
Loopflow Home received its own database and runs. This proves authentication for
that turn, not live connection, present capacity, or repeatable fixture cleanup.
The dated recipe and account side effects remain in `scratch/questions.md`.

A completed headless Flow-step engine is a candidate fixture, based on earlier
observed surviving engines; a plain batch run closed its engine. Fixture creation,
owned-process tracking, retained drafts and safe repeated connection remain
unimplemented. Claude remains unmeasured; support and availability must be reported
separately. No authentication blocker remains established by the supplied evidence.

Check: source inspection at `58d3b5b3e` and `git diff --check` — plan reconciled with upstream; prose-only changes, no connection benchmark or tests rerun.
