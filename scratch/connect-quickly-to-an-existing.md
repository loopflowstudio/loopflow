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

## Implementation evidence and revised design — October 5 UTC

`tests/e2e/codex_connect.py --connect-performance N --output <new-dir>` now
launches production `lf session connect` under an owned PTY, using a real Codex
engine/native UI, synthetic local Responses and disposable Loopflow/provider
Homes. Each sample starts a fresh held seed turn. It records driver claim,
recognizable output and a unique input-response marker separately, preserves
failures, checks recorded Session/thread/PID/generation and the OS birth stamp,
and stops owned processes. Failed samples stop the run with a nonzero exit.
It has no configured-account dependency. These are harness smoke checks, not
retained-history or authenticated-provider acceptance.

The native baseline exposed a concrete failure: the resume command adds local
permission overrides, which Codex 0.160.0 rejects on remote resume. The bounded
production repair builds remote resume without local permission/model/workspace
overrides; the live engine owns those values. Ordinary stopped resume remains
unchanged. Two successful native probes reached output and a submitted response;
the latest also checked the actual engine birth stamp. Failed attempts remain in
[the dated evidence](../scripts/benchmarks/session-connect/20261005-native-smoke/README.md).
The first probe timed out on disposable folder trust and its cleanup failed;
a later probe separated text/Enter to avoid Codex paste handling. No installed
Home, account route or unrelated Session was changed by these probes.

### Lifecycle counterexample: dependent optimization stops

The baseline's rejected UI also cleared the pre-existing engine endpoint.
`connect_live_codex` claims the driver before native UI startup, then calls
`finish_session_driver` even when startup fails. The supported close operation
ends the live engine. Removing one rejected flag repairs its observed trigger,
but does not meet the accepted preservation contract for other startup failures.
This is a reproduced contradiction in the attachment lifecycle, not an auth,
display-server or performance-budget blocker.

The design must distinguish unsuccessful UI attachment from an established
controlling UI's ordinary exit, using the existing Session/driver authority.
A candidate is preparing the native UI without transferring write authority,
then transferring at a provider-confirmed boundary; that is a proposal, not an
accepted change to takeover semantics. It must retain driver fences and survive
failure without restoring stale authority. Merely skipping all driver cleanup
would abandon current exit semantics and is not an adequate repair. Dependent
latency optimization and delivery depend on this lifecycle revision; local
implementation remains authorized.

Infrastructure's October 4 review-replacement evidence (LOO-377, merged
`c5dc238b0afb`) separates review service, driver and provider ownership. Its
launch-lock-before-driver-fence ordering and exact Flow revalidation apply to
the Flow-review repair. Attachment must not acquire review-completion authority
or turn failed startup into successful settlement. A substantial lifecycle
change remains implementation work, not a bounded reconciliation edit.

Flow-review selectors have another unresolved path: lock, repeated position
validation, projection, then `resume_native_session`, which stops native clients
and passes no live relay endpoint. The new runner does not exercise that selector
yet. Preserve its revalidation/review token and prove existing provider/draft
continuity when adapting it. A conversation-only success cannot stand in for
Jack's exact selector. Claude/OpenCode remain unmeasured; their absence from the
Codex relay path is not an all-provider support verdict.

### Remaining implementation and acceptance

- Revise and implement failed-attachment preservation, then exercise failed and
  successful native startup against a retained controlling client and draft.
- Extend the production PTY runner to exact Flow-review selectors, completed
  short/paginated history, repeated connections, retained drafts and supported
  authenticated providers. Keep unsupported and unavailable routes explicit.
- Capture comparable baseline samples and contributing startup/lookup/transport
  costs before choosing numeric median/p95 targets. At least 20 comparable samples
  are required for p95. The five changing smoke probes establish no distribution
  and justify no latency target or optimization claim.
- Implement file-based per-invocation diagnostics, keyed to exact Exec, selector
  and resolved Session, with a supported reader. Reuse `journal::process_elapsed`
  for in-process phases and the runner's external spawn clock for endpoints.
  Keep lookup, connection preparation, first output, verified input and attached
  lifetime separate; flush while attached. Unobservable endpoints need an
  explicit unavailable reason. SQLite contention can hide Exec receipts, so the
  diagnostic must not depend on a successful database write. No second lifecycle
  store is proposed; the supported reader and observation boundary remain design.
- Only then optimize measured redundant work, verify comparable before/after
  results, and complete the existing acceptance and authorized delivery.

Delete — do not maintain: remote resume no longer applies local launch overrides.
No history-recovery deletion is justified. `recover_history` still awaits every
page before UI startup; moving it requires proof of history ordering and readiness.
Preserve ownership fences, review revalidation, historical data and failure evidence.

The probe uses recorded endpoint timestamps instead of duplicate sent/released
flags, closes each SQLite poll connection, and stops polling after driver claim.
This reduces observer work; new timings are not directly comparable with the old
polling loop and establish no production speedup.
The subsequent smoke sample reached output at 856 ms and a response at 1528 ms
with the same engine birth stamp. The response is an upper bound on input
readiness, including 300 ms before Enter and synthetic response work; it is not
the instant the UI first accepted input. Continuity is checked before cleanup,
so this success establishes neither attached lifetime nor detach preservation.

Review findings: protocol fixtures previously replaced the native UI and missed
its rejected launch arguments. A successful socket/driver claim did not establish
usable input. The failed native UI also exposed destructive cleanup; timing alone
would hide that regression. PTY bytes are not compositor presentation, synthetic
responses are not provider service latency, and a single live seed turn is not
representative retained history. Product has no child Wave memories in this checkout.

Check: prior build/focused Rust test/fmt/Clippy/helper Ruff passed; compression's `uv run tests/e2e/codex_connect.py --lf target/debug/lf --codex /Users/jack/.local/bin/codex --connect-performance 1 --output /tmp/loo378-compress-20261005a` passed (856/1528 ms); broader runner Ruff reported eight pre-existing E501 lines; reconciliation `git diff --check` passed; full acceptance remains with implementation/gate.
