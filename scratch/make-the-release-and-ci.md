# LOO-326: recover from transient faults without rerunning the work

Approved implementation plan — 2026-10-02. Reconciled against `101a19b8c`.
Jack Heart approved the design, including valid cached planning regardless of
age, waived a personal demo, and requested an autonomous Flow on 2026-10-02.
Automated acceptance and delivery checks remain required. This authorizes Task
delivery, not a product release or installation replacement.

## Problem and acceptance

Jack can cut a release or restart existing work without a transient transport
failure forcing a whole-operation rerun. Test fixtures cannot spend hours waiting
on external services. This serves Infrastructure's execution-continuity objective
and the chapter KR that Tasks advance without plumbing intervention; no numerical
chapter target was supplied.

Automated acceptance uses disposable repositories, a private Home and scripted provider
boundaries. One release invocation encounters a partial artifact download and a
GitHub 502, recovers, and prepares the same candidate. With Linear unreachable,
`lf task restart FIX-1` replaces its saved Flow and reaches the first authored
review boundary, retaining the Task, checkout, PR and prior history. No real
release, installation replacement or live Task restart is needed for acceptance.

## Reconciliation and concrete findings

| Fault | Evidence in this checkout | Remaining obligation |
| --- | --- | --- |
| Swift cancellation hangs | `632b67ec7` / PR #1311 is an ancestor. `ActiveRunsObservationTests.swift` uses bounded async exit observation; cleanup terminates a running child without duplicate blocking joins. Jack's later steer establishes v0.12.24 shipment and passing hosted checks. | Preserve the fix and run the complete transport tests at gate. The unavailable original hosted stack remains unknown. |
| Candidate artifact download fails once | `prepare_publisher` in `ops/release.rs` invokes `gh run download` once into a temporary directory. Publisher preparation follows only on success. Exact-source materialization is already present. | Retry transient reads within that invocation, without retaining partial download files. |
| Restart waits on Linear | `restart_task_async` forces `resolve_managed_task_planning(..., Force)` before checkpoint/stop. The replacement's managed executor in `lf/commands/flow.rs` repeats planning admission with `Auto` before each tick. | Remove remote refresh from both existing-work admission boundaries; replacing only `Force` leaves the worker blocked later. |
| Telemetry test fetches over SSH | `4439aedd9` / PR #1349 is an ancestor. `doctor_tests::run_lf` binds Git to a disposable repository and sets `GIT_ALLOW_PROTOCOL=file`. Current Doctor additionally reads cached upstream facts without fetching. | Preserve the fixture isolation. Audit and prove the broader suite requirement; this single repair does not establish that all tests avoid external network. |

Additional dropped-connection evidence from Jack concerns `gh pr checks
--required` returning HTTP 502. That literal command is no longer the check
reader: `ops/pr.rs::merge_gate_state` reads paginated GraphQL through
`read_check_page`, whose failed subprocess still immediately propagates an error.
Cover the current boundary, retaining exact-head checks on every page.

Planning already has the necessary local representation. `PmRefresh::Never`
returns persisted normalized observations without fetching. The managed reader
accepts only `PlanningState::Available`; `resolve_managed_task_planning` also
checks terminal state and matching Task, Project, Wave and repository ownership.
Age is separate from validity; a local read does not make old facts fresh.
`TaskInput::refresh` already polls comments independently and retains confirmed
direction when refresh fails. Restart with new advice is different:
`publish_task_steer` publishes to Linear first. Do not accidentally promise an
offline write or drop that advice.

The separate startup diagnostic remains evidence in
`loo-326-worker-startup.md`. Its deleted tmux-server cwd is a fifth fault, not a
substitute for these acceptance checks. No startup workaround belongs in this
implementation, and no replacement live worker is launched by this plan.

## Chosen approach

### 1. Bounded release reads

Keep `gh` as the transport and the existing release state machine as the owner.
Add a small internal bounded-read function at the operations layer, shared only
by artifact download and the GitHub check-page read. No global retry of arbitrary
commands, release stages, publisher writes, tagging or PR mutations.

Use three attempts with 1s then 2s backoff. Retry read timeout, connection reset,
unexpected EOF and HTTP 502/503/504. Authentication, permission, missing artifact,
malformed JSON, validation and signing errors fail immediately. Unknown failures
remain errors. Classify captured stderr narrowly because `gh` exposes these
transport failures as subprocess errors; keep observed strings as fixtures.

Each download attempt owns a fresh TempDir. Return only the successful directory
to publisher preparation; failed files are dropped before another attempt.
Retain the exact candidate tag, commit and workflow run ID across attempts. Never
retry preparation merely because a previous download failed. Keep existing
manifest/checksum and exact-source validation unchanged.

Bound each download attempt to five minutes and each check-page read to 30s.
Three exhausted attempts therefore take at most 15m3s or 93s, plus bounded child
cleanup. Use owned child termination/reaping with concurrently drained output;
the current `Command::output` wrapper alone supplies no timeout. Implement this
at the read helper, reusing the existing process-group cleanup primitives, not
by changing every subprocess in the program. Test timeout cleanup with a local
child that never exits and a descendant holding an output pipe open after its
parent exits. The deadline covers output collection as well as process exit;
cleanup must not wait indefinitely for pipe EOF. A locally enforced read deadline
is a retryable timeout within the same three-attempt budget. Log operation,
attempt and terminal cause without signed
URLs, credentials or full sensitive transport output.

Retry an individual check page. Accumulated pages are usable only while each
response still matches the expected head; changed heads return missing evidence
as today. Never turn exhausted transport retries into a passing or empty gate.

Focused proof: extend `release_tests.rs`'s existing fake-gh/publisher fixture so
the first download writes partial bytes and fails, then succeeds. Assert one
prepared candidate with only successful bytes and unchanged identity. Add
permanent-failure, exhaustion and owned-child timeout cases; test GraphQL 502
followed by a matching page and changed-head rejection in the existing PR tests.

### 2. Restart from confirmed local planning

For an existing Task, use the normalized local observation through
`resolve_managed_task_planning(..., Never)` for restart, continuation and managed
executor admission. Preserve all existing validity and ownership checks. Remote
acquisition remains in explicit planning refresh, initial Task resolution and
provider observation; publication/completion keep their own authority checks.

Select replacement Flow from the validated cached Project, or the explicit
`--flow`, and compile it before stopping existing work. Preserve checkpoint,
exact worker stop, claim fencing, old Flow history and `restart_task_flow`'s
transaction. Update derived Task/Project snapshots only from the selected
observation and retain its actual observation timestamp.

Absent, invalid, removed, terminal or mismatched records refuse before checkpoint
or stop and name the planning problem. An old but otherwise valid observation
can admit existing work, while status continues to show its age. This is the
outage policy approved by Jack Heart for this Task on 2026-10-02; it does not
change unrelated planning policy. It cannot discover a remote change during an outage;
new locally received invalidation must block the next boundary.

Bare restart is the required offline operation. Supplying new advice continues
to require successful publication before replacement; on failure, preserve the
old worker and Flow and clearly say the direction was not delivered. Do not add
a second local steer source, outbox, silently discarded advice or success claim.
The current command checkpoints and updates the Project snapshot before advice
publication. A failed publication therefore preserves execution but may leave a
local checkpoint; diagnostics and tests must not promise a completely unchanged
checkout. This repair does not require transactional rollback of that checkpoint.

Proof must cross the public restart command and replacement execution, not only
the store method: a previously acquired valid Task with an old observation,
unreachable Linear and a local fake agent advances to the authored review stop.
Compare Task/Project/Wave IDs, worktree, PR, prior Flow history and new Flow ID.
Repeat with invalidation, removal, terminal status and ownership mismatch; those
must retain old execution. Confirm publication is still governed by its existing
checks. Include advisory-publication failure before destructive restart effects.

### 3. Finish the network-free test obligation

Keep the shipped doctor and Swift repairs. Audit subprocess and HTTP entry points
in default Rust, Python and Swift suites, including inherited Home, Git config
and compiled-source paths. Replace external service dependencies with existing
local fixtures/fake executables; real Git semantics use disposable bare remotes.
No production offline switch or test-only provider abstraction.

Apply file-only Git transport to the test commands in `scripts/test.py` and CI,
not developer shells. Update any fixture that overrides it so it cannot restore
SSH/HTTPS accidentally. Denying Git alone is not the whole-suite proof: gate/CI
must also execute prepared test binaries with external egress denied, allowing
loopback for local protocol fixtures. Dependency/image acquisition and builds
run before that boundary. Use the capable runner's network isolation (Linux
container without external networking; macOS test-process sandbox), with an
explicit denial probe before the test run. Unsupported isolation is reported as
deferred evidence, never a passing network-free claim. Do not remove behavioral
coverage or add retries to flaky tests to achieve this.
Prove both external denial and working loopback inside the exact test-process
boundary. The existing Desktop sandbox denies WindowServer, not network access;
reuse its headless path without treating it as an egress-denial proof. Include
the descendant processes launched by fixtures in the isolation audit.

## Ownership, alternatives and exclusions

The release candidate/receipts remain authoritative for tag, source and workflow;
temporary downloads are disposable input. SQLite planning observations own known
provider facts; Task and FlowSession records retain execution and exact claims.
No schema, wire DTO or Swift production API change is planned. CLI help/docs must
describe restart's cached planning and new-advice boundary; TESTING.md owns the
test isolation instructions. Review only necessary observability changes in the
existing status envelope, without adding a second freshness representation.

Rejected alternatives: whole-release retry can replay writes; downloading into
the same directory can mix partial attempts; a larger Linear timeout still
couples local restart to availability; blindly using `Task.plan` bypasses
invalidation; replacing only the restart refresh fails at worker admission;
test timeouts limit damage but do not remove external dependencies.

Delete the one-shot download call and forced/automatic provider acquisition at
the existing-work admission sites when their replacements land. Delete any
external-network fixture discovered in the audit, replacing its behavior proof
in the same change. Preserve provider mutation authority, exact-source release
validation, same-tag recovery and all shipped cleanup tests. No Legacy/New path,
additional store, generalized deployment framework or compatibility shim.

Exact-source candidate validation and invalid-tag recovery remain with the
run-records work identified by Jack at `424958e9d`; this Task does not redesign
them. Live releases, tmux startup repair, installation selection and global
network/SSH settings are excluded.

## Implementation reconciliation — 2026-10-04

Jack Heart explicitly authorized autonomous publication and landing with Task
completion, without an interactive review. Existing authored work and the dormant
old review client remain preserved. No real release or installation replacement.

Implemented bounded GitHub download/check reads and cached planning at restart,
continuation and managed admission. Focused release proof recovers partial bytes
in one invocation and prepares the original candidate once. Public restart proof
reaches review through a mechanical step with unreachable Linear, retains identity
and history, and rejects invalid/removed/terminal/moved planning and failed advice.
A mechanical step exercises worker admission without unnecessary provider setup.

Network isolation uses per-process macOS sandbox or Linux network namespace;
loopback and denial probes run inside each boundary. Dependencies/builds precede
test execution. Remaining work: repair concrete suite failures under isolation,
finish headless gate, prepare accurate PR copy and land through typed operations.

## Done when / gate

Run headlessly once on the implemented tree:

- `cargo test -p loopflow --test release_tests`: transient and partial download
  recovery succeeds in one invocation; permanent failures/exhaustion preserve
  release state; publisher sees only successful bytes for the original candidate.
- `cargo test -p loopflow --lib ops::pr::tests`: transient check reads recover;
  malformed results, changed heads and exhausted reads never authorize landing.
- `cargo test -p loopflow --test task_restart_tests` (new public-path fixture):
  valid cached work restarts and reaches review with Linear unavailable; all
  invalid-state preservation cases and advice failure pass.
- `cargo test -p loopflow --test doctor_tests` and
  `scripts/test_desktop.sh --filter ActiveSessionsObservationTests`:
  retain the telemetry behavior and complete transport suite, including real
  CLI cancellation, through the headless path. Display-dependent native tests
  remain optional capable-runner diagnostics, not headless prerequisites.
- `uv run python scripts/test.py --all` on capable CI with prepared dependencies
  and external egress denied for test execution: no test waits on an external
  service. Record any platform isolation gap explicitly; container image pulls
  and dependency resolution are setup, not tests. Existing headless Desktop
  limitations remain assigned to capable CI rather than concealed as success.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.

Review finding: a command returning a replacement Flow ID is insufficient;
worker admission has its own refresh. The acceptance scenario therefore reaches
the authored review boundary and inspects retained history. A recovered release
must also preserve artifact identity and exact-head evidence, not merely exit 0.

Check (2026-10-04): read retry (3), partial-artifact release (1), public restart (6 scenarios), PR checks (37) passed; isolated Python repair suite passed 54 checks and all three final command-plan assertions passed; full gate twice stopped before tests (resource probe timeout, then 29.7 GiB below 32 GiB reserve), so final Rust/Swift/Python matrix and Clippy defer to hosted CI.

Review findings fixed: the read deadline includes pipe EOF and child reaping;
restart admission now has one cached-policy owner; failed advice names preservation
explicitly. macOS network isolation reuses the fixed ps reader exception required
by Desktop. Local evidence predates final small diagnostic/cleanup edits; CI must
validate the landing candidate. No product release or installed Home was touched.
