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

## Remaining design and implementation — October 5 UTC

Ordinary reconnect loses the native UI's unsent draft. The disposable runner
observes that draft in the original UI, connects again without `--replace`, then
submits a fresh marker from the second UI. The accepted request contains the
marker but not the draft, despite unchanged Session/thread/generation/PID/birth.
Driver transfer and engine reuse cannot satisfy Jack Heart's continuity contract.
[The reconnect evidence](../scripts/benchmarks/session-connect/20261005-reconnect/README.md)
retains the submitted input, earlier inconclusive attempts and cleanup timeouts.

**Proposal, not implemented:** retain the native UI and its PTY across connections.
Establish terminal ownership at first launch for conversations and reviews, then
attach presentation without creating another UI/provider or transferring the
Session driver. Additional views stay passive until explicit control transfer.
The terminal owner must handle replay, resize, input fencing and failure cleanup;
view closure must be separate from provider shutdown. Do not scrape or retype
composers or introduce a parallel draft store. Already-running standalone native
clients without an attachment transport cannot be silently converted.

The normal Flow-review route launches `--mode tui` through `serve_flow_locked`
→ `run::exec_prompt` → `exec_session_with_env`, without the batch Harness's relay.
`launch_flow` → `start_durable_session` already provides a tmux cradle, but
`resume_native_session` stops and recreates its native UI. Direct terminal launch
has no retained transport. Routing reviews through `connect_live_codex` alone
falls back to that replacement; the attempted adaptation was removed.

The [October 5 terminal comparison](../scripts/benchmarks/session-connect/20261005-terminal-transport/README.md)
now supplies bounded evidence, not a selected production transport. A private
configured tmux server preserved truecolor/paste bytes, three reattachments,
fixture draft submission and explicit input transfer. A 60-column passive view
left the application's 100×30 PTY intact. Cleanup observed both server and fixture
exit. This is a canned application, not native provider continuity or image proof.

Two counterexamples invalidate a plain-attachment/raw-replay implementation:

- With `get-clipboard request`, tmux directs an independently emitted clipboard
  query to the recently active passive viewer. The controller gets no query;
  the passive reply does not reach the application. Controller replies work
  before passive attachment and after explicit transfer. Read-only typing alone
  does not establish ownership of terminal responses.
- Raw transcript replay contains historical clipboard queries. Responding to
  those queries writes fresh clipboard data into the retained application PTY
  unless the owner suppresses replayed queries and fences replies. This is a
  counterexample to the proposed raw relay, not an existing production vulnerability.

**Revised design requirement, not implemented:** one retained terminal owner
must hold application screen state and live terminal queries separately. Late
views receive a display snapshot plus subsequent display updates, never past
clipboard/device queries. Only the controller supplies query replies and PTY
size; passive viewers have independent cropped/panned presentation. Transfer
revokes the old input/reply path before granting the new one, without changing
the Session driver. Full view-specific reflow is not promised by one PTY.

Plain tmux attachment and an append-only byte replay are rejected as the complete
owner. A tmux-backed presentation needs explicit controller-bound query routing;
a transparent relay needs terminal state/replay handling, input fencing and bounded
backpressure. The comparison does not choose between those implementations.
This is a substantial remaining architectural cut, not a latency optimization.
Dependent production replacement remains stopped on these counterexamples.
No new user authorization or manual demo is needed to implement the revised owner.

Preserve Infrastructure's LOO-377 launch-lock-before-driver-fence ordering and
exact review revalidation (`c5dc238b0afb`). Attachment grants no review-completion
authority. Keep captured input, review tokens, provider/driver generations and
history intact; failed startup cannot settle a review.

### Delete — do not maintain

- In the retained-terminal cut, remove live attachment through
  `resume_native_session` that stops/recreates the UI. Supply the replacement in
  the same cut; preserve stopped resume and explicit replacement as distinct
  operations. Do not polish that predecessor or its exclusive fixtures meanwhile.
- Local launch overrides on remote resume are already removed. The live engine
  owns workspace, model and permission policy.
- Retain `recover_history`. It currently awaits every page before UI startup;
  changing that requires history-ordering and readiness proof, not just faster
  new-UI startup.

### Acceptance still open

- Prove Jack's exact Flow-review selector with retained native draft/history,
  failed startup and exact review completion/retirement fences.
- Prove successful and repeated retained-UI attachments, including detach
  behavior; complete short/paginated history and supported authenticated-provider
  coverage. Claude/OpenCode and standalone-native attachment remain unmeasured.
- Capture comparable baseline samples and contributing lookup/startup/transport
  costs. Select numeric median/p95 targets before optimization; p95 requires at
  least 20 comparable samples. Then fix measured costs and compare on the same
  host/data, preserving failures, timeouts and observer limitations.
- Verify per-invocation diagnostics on the revised review path, keeping unavailable
  UI endpoints explicit. Complete acceptance before authorized delivery.

## Existing safety and diagnostic evidence

The runner uses real Codex engine/native UIs, private Loopflow/provider Homes and
synthetic local Responses. Each sample begins with one fresh held seed turn.
It checks engine identity including OS birth stamp, records output and response
markers separately, and exits nonzero on failure. It needs no configured account.
These are smoke checks, not representative history or provider-service timings.

[Native smoke evidence](../scripts/benchmarks/session-connect/20261005-native-smoke/README.md)
records Codex 0.160.0 rejecting local permission overrides during remote resume.
That failure also exposed destructive cleanup: claiming the driver before UI
startup let a rejected UI close the pre-existing engine. Remote resume now omits
local overrides; stopped resume is unchanged.

[Attachment evidence](../scripts/benchmarks/session-connect/20261005-attachment/README.md)
records the lifecycle repair. The relay claims authority only after delivering
the matching successful thread-resume response. Failed bootstrap retains the old
driver/engine/UI; the runner submits the exact retained draft after a rejected
replacement. Bootstrap reads stay passive; writes and approval replies retain
driver fences. Explicit replacement snapshots old clients and stops only those
clients after attachment. Current-owner exit still closes its engine. Protocol
fixtures separately prove stale-write rejection and sibling-thread preservation.
Neither protocol attachment nor failed-replacement proof establishes successful
reconnect continuity.

`lf session timings EXEC` reads file-backed JSON lines without SQLite admission:
selector, resolved Session, process-relative phases, unavailable output/readiness
endpoints and attached lifetime through return or handled interruption. Output
arrives under an exclusive SQLite lock, though accounting can delay reader exit.
SIGKILL may leave lifetime unknown. These observations grant no lifecycle authority.

The attachment smoke recorded 652 ms output, 1,149 ms input response and 998 ms
attached lifetime. Response bounds readiness rather than measuring onset; it
includes 300 ms before Enter and synthetic tool work. Attached lifetime includes
post-input proof. External PTY timing starts before spawn, internal phases at
process entry. PTY bytes are not compositor presentation. Changing probes and
reduced observer polling make these samples unsuitable for speedup/distribution
claims. No Desktop KR credit follows.

Review found that protocol-only fixtures missed native argument rejection and
that cleanup exceptions could hide the behavioral failure. The runner now retains
cleanup errors separately and attempts every owned cleanup. Reconnect cleanup
still timed out; the later exact-path inspection found no remaining final-probe
lf process, which does not prove reliable cleanup. Jack's live Session and
unrelated processes were not touched.

Earlier design chronology and observer notes are preserved at
`42c4c791cea8d8cb07f8ee545c2b83194bf8fc63:scratch/connect-quickly-to-an-existing.md`.
The transport comparison now centralizes attachment registration and detach/wait,
so every view shares cleanup ownership. Review retained explicit controller-first
revocation and both clipboard counterexamples. The native replacement path and
its failing regression remain untouched pending the retained-terminal cut.

Check: `uv run python tests/e2e/terminal_transport.py --output /tmp/lf-terminal-compress-20261005-01`, runner Ruff check/format and `git diff --check` passed; three reattachments and owned cleanup passed, clipboard/replay counterexamples retained; native acceptance remains with implementation/gate.
