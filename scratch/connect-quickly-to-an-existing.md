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

Jack supplied this reproduction: lf session connect task_7c24c806bfaa464a877352b568384fff:6430c57b-02e4-4ca9-9021-afb5e68c9003:feature:review_kickoff:0. Recorded connect Exec 360dfe3c-8962-47cf-aab8-c4936ddde21a began October 4 at 10:46:14 America/Los_Angeles (1791135974). Jack observed many seconds of delay. At the recorded inspection the command remained attached; its command record has no separate connected/usable timestamp, so exact latency is unknown. Include this Flow-review Session selector path in reproduction, even if it resolves or launches rather than attaching to an existing provider. Preserve the live Session; reproduce with isolated equivalents.

Additional acceptance: retain connect phase timings keyed to the exact invocation and Session, and expose them through a supported diagnostic command/log view so Product can answer how long a specific connect took afterward. Separate lookup, preparation/connection, first output and verified input readiness from total attached lifetime; record unavailable readiness evidence explicitly.

## Implemented retained owner — October 5 UTC

The private tmux server now retains the native UI and application PTY for prepared
conversations and Task Flow reviews. Only the controller attaches as a tmux client.
Passive views poll `capture-pane` screen snapshots, keep one frame at a time with
a 4 MiB bound, and crop/pan without resizing the application. Historical terminal
queries never enter those snapshots. Explicit `--take-control` detaches the old
client before attaching its replacement; no Session driver changes. View detach
is separate from native exit/completion. This is an engineering selection within
Jack Heart's accepted continuity contract, not a new product decision.

Exact Session identity plus canonical Home determines the private socket. New
launches use the existing service process and launch lock; presentation may attach
while that service waits for startup input. Current review lookup still validates
the exact captured boundary. Stopped resume remains native. Already-live standalone
UIs without this transport are left unchanged; direct standalone launches do not
yet acquire a retained owner. No draft scraping, retyping or parallel draft store
was introduced. The existing engine-only attachment path is deleted.

Two implementation counterexamples changed the cut: a redundant controller-lifetime
lock prevented transfer, so tmux's client registration owns control; exact Session
IDs must resolve before artifact-key parsing, because ordinary IDs can look like
artifact keys. The private native probes exposed startup trust prompts behind the
launch lock. Early presentation attachment supplies input without claiming driver
or review authority. Failed non-TTY attachment is checked before detaching a client.

## Evidence and performance

[Retained-terminal evidence](../scripts/benchmarks/session-connect/20261005-retained-terminal/README.md)
contains native results, failed attempts, terminal bytes and per-invocation timings.
Twenty eight-turn review reconnects establish the baseline: output median 86.3 ms /
p95 92.0 ms, composer response median 87.6 ms / p95 93.7 ms. Before optimization,
both endpoint budgets were set to median <=100 ms / p95 <=125 ms. Removing the
redundant tmux existence process gave output 81.7/89.7 ms and response 83.5/97.1 ms.
Both budgets pass; the input tail worsened and test compilation overlapped that
comparison, so no reliable tail speedup is claimed. These are fixed synthetic
populations on the same host/debug profile, not authenticated-service or Desktop
measurements. The incompatible earlier UI-replacement probe is not a baseline.

Native conversation and review probes preserve PID/birth, Session/capture/driver,
history prefix, visible response and unsent draft across twenty detach/reconnects.
Passive typing is ignored, explicit takeover preserves the driver, final submitted
input contains the original draft, and completion removes the owned native UI and
server. Review Complete fails before Ready and settles its exact capture afterward.
The native review fixture ends at that review: the earlier full-feature trial saved
feedback but failed to start its next worker without a managed account in the
isolated Home. This failure and earlier cleanup/observer failures remain retained.

`lf session timings EXEC` now records preparation/attachment/lifetime on retained
conversation and review paths. Native output/readiness stay explicitly unavailable
in the diagnostic; the external PTY runner observes screen bytes and composer
responses separately. Response bounds readiness; it does not measure onset.
File-backed diagnostics remain readable under SQLite write contention. Accounting
can delay command exit and SIGKILL can leave lifetime unknown.

The [original transport comparison](../scripts/benchmarks/session-connect/20261005-terminal-transport/README.md)
rejected ordinary read-only tmux clients (passive clipboard-query interception) and
raw replay (historical queries). The snapshot variant resolves both at the fixture
boundary. It does not prove native images. Prior engine-reuse draft loss and
failed-bootstrap ownership evidence remain in the dated reconnect/attachment
benchmark directories and `5e0b575b6:scratch/connect-quickly-to-an-existing.md`.

## Remaining acceptance

- Extend retained ownership to direct first-launch conversations, or establish an
  explicit supported-path boundary without claiming all live Sessions attach.
  Existing standalone native UIs cannot be converted without replacement.
- Prove authenticated supported providers, Claude/OpenCode, native image input and
  paginated/very long history. Eight synthetic turns preserve history but do not
  exercise pagination. Account routing/installed Homes must not become hidden
  prerequisites. No configured account is available in the isolated review fixture.
- Prove continuation into the full feature Flow's next worker, beyond the proven
  final-review settlement and existing exact-boundary Rust regressions.
- Prove failed native startup and SIGKILL recovery on the retained path.
  Concurrent opens now prove one controller; SIGTERM of an unread passive view
  preserves the UI. Passive writes have a two-second no-progress bound, but a
  saturated-output stress probe remains distinct from that termination test.
- Complete affected gate checks and reconcile CLI/Desktop action presentation for
  the retained path. Publication and landing remain after complete acceptance.

## Delete — do not maintain

Completed: removed live `connect_live_codex` UI replacement/driver transfer,
`make_session_interactive`, the relay's attachment-claim callback, and their
exclusive engine-reuse probe machinery. Full Session IDs replace truncated
background names. Preserve stopped native resume, explicit standalone replacement,
review capture/driver fences and `recover_history`; none grants live attachment
proof. No compatibility flag or alternative production transport remains.

Check: native conversation/review baselines and lookup comparisons (20/20 each), concurrent/unread-view review smoke (3/3), snapshot fixture, 33 human_session tests, four terminal tests, exact-review regression, build, fmt, Ruff, all-target Clippy and diff check passed; affected gate and remaining provider coverage are still open.
