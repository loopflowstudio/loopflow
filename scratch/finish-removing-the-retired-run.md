# Finish the Run cutover — LOO-370

Draft for feature Flow design review, 2026-10-02. Jack Heart requested cleanup
while preserving conversations, history and work. This plan is not approval of an
installed-Home migration. Base: `a278d6bc1bd4373f78f27027b8ae249100ef14d3`.
The authored `feature → task-design` Flow puts `review-design` with `human: true`
after kickoff and before implementation. Production implementation remains pending.

## Problem and observable outcome

Maintainers still encounter capture keys called Run IDs, execution policy selected
by environment-variable presence, and Session payloads beneath `runs/`. This makes
it easy to target the wrong identity or overlook live history during cleanup.

After this change, new and resumed conversations keep their AgentSession identity;
captured inputs remain subordinate history, FlowSession owns progression and Exec
owns command ancestry. Commands, Desktop, replay, usage and agent instructions use
those owners consistently. No generic Run/attempt entity replaces the removed one.
This serves Infrastructure's continuity/reduction objective and the chapter's two
unchecked KRs: ordinary Sessions can advance work without plumbing intervention,
and retained Sessions/reviews do not need manual rediscovery. There are no current
chapter metric targets; source reduction is supporting evidence, not a won KR.

## Demo

In a disposable OS account/container, start with a populated released Home and
known Session/review IDs. Observe `lf session list --json`, `lf mon show <input>`
and `lf usage --json`. Attempt capture conversion while a released provider writer
is alive: conversion reports that writer and leaves all data/selection unchanged.
After controlled quiescence, convert through the candidate's installation recovery
path, interrupt it after the directory move, then retry. The same commands show
the same history, feedback, usage and native conversation identity. Resume that
Session, run a nested direct skill and an agent-issued command, and complete the
saved review through `lf session ready` / `lf session complete`. One Flow advances
once; new bytes appear only under `captures/`; no `runs/` alias remains.

This is a headless fixture demo with provider stubs. Real installed/provider proof
is separate and requires authorization; neither kickoff nor Task creation grants it.

## Findings and inventory

Inspection covered tracked Rust, Swift, Python, scripts, builtin Skills, current
docs/configuration and migration fixtures. This is the starting inventory, not a
claim that every remaining reference has been removed. Final inventory belongs
beside the architecture documentation when implementation is complete.

| Current coupling | Actual owner and planned disposition |
| --- | --- |
| `durable::RUN_ID_ENV`; `session_record::{RUN_DIR_ENV,inherited_caller,record_dir}` | Values are capture artifact key/path, not Session or Exec IDs. Remove old exports/readers; keep one validated capture context for artifact association only. |
| `session_record::CaptureHandle::environment`, `lf/commands/util.rs` native resume | Both launch paths must install consistent Session/driver provenance and capture context; native resume has no CaptureHandle. |
| `exec::AgentCaller`, `journal::create_exec_context` | Existing Session/provider-generation/origin-Exec provenance is the causal authority source. Journal consumes/removes `LF_AGENT_CALLER`; later consumers must read resolved context, not reread the environment. |
| `bin/lf.rs` direct checkpoint; `ops/task/lifecycle.rs::branch_task` | Replace presence checks with explicit resolved caller/work facts. Top-level unbound skills retain automatic checkpointing; nested/shared contributions leave composition to the caller. Registry failure under owned work remains an error. |
| `ops/linear_observe.rs::publish_issue_comment` | Agent progress provenance comes from resolved captured input/Session association. Preserve old comment markers as immutable evidence and their steer exclusion. `--steer` remains deliberate direction. |
| `ops/human_session.rs::{active_run_id,find_session,owned_target}`, `human-present.md` | AgentSession identity selects rename/ready/complete; capture identity is never substituted for it. Preserve exact review invocation, position, successful native completion and driver fences. Historical input selectors remain read selectors, not current authority. |
| `durable::FlowAttempt.run_id`, `store/sqlite/sessions.rs`, `lf/commands/flow.rs`, `ops/run.rs`, `lf/commands/run.rs`, replay | Artifact names/variables become capture names. Prefer existing captured sequence where sufficient; collapse the transient FlowAttempt projection if its fields already come from the selected capture. Delete the FlowAttempt wrapper if selected captured-event data makes it redundant; otherwise rename this transient projection to SelectedCapture and its key field to artifact_key. It gains no table, ID or lifecycle. |
| `session_record/active/reader.rs` | Hard-coded root/depth in scan, receipt classification, watch reconciliation and rescans. Cut all to one capture-layout function together. Preserve exact native-client process evidence and bounded scans. |
| `ops/git_operation.rs::GitOperationOwner.run_id` and `process_id` | Actually Trace/Exec evidence; rename fields to their owners. Preserve live sequencer identity/lock and historical receipt decoding at the conversion boundary; never recreate sequencer authority from ancestry. |
| `engine/event.rs::EngineEvent`, `engine/error.rs::{RunNotFound,StepRunNotFound}`, `engine/worktrees.rs::short_run_id` | Repository symbol search found definitions only. Delete unused events/helper/error variants and exclusive exports/tests, rather than modernizing an unused event bus. Check public export impact explicitly. |
| `engine/{agent,process}.rs`, harness, release/install environment scrubs; Swift `ProcessEnvironment` | Synchronize actual capture/Session/Exec context exports and scrub lists, including tmux propagation; old names may survive only as inert sanitization of inherited historical shells, never as execution input. |
| Swift `TaskRunsView`, `TaskRunsProofTests`, `RegistryQuery`, `SessionRecord`, launcher comments | Name the rendered work/history projection after its content. CLI response/DTO fixtures must agree across Rust and Swift with required/Optional fields, no silent defaults. |
| `scripts/context_ablation.py` | Current default and staging/replay paths use `runs`, with duplicated artifact path logic and old manifest normalization. Cut live/replay paths to captures and shared public history selection; retain explicitly historical cohort decoding only where it still has a real reader. |
| `scripts/loopflow-dev.py`, fixture support scripts, environment/architecture docs | Remove `lf runs` guidance, misleading identity names and stale capture-root prose; update command tests and exported builtins together. |
| `engine/error.rs::LoadError` | `lf list` discovery hint is stale; use the current uniquely resolving catalog command. |

Search vocabulary includes `LF_RUN_ID`, `LF_RUN_DIR`, `RUN_ID_ENV`, `RUN_DIR_ENV`,
`run_id`, `parent_run_id`, Run-prefixed types, `lf runs`, and literal `runs` roots.
Search tracked files, excluding generated `.lf/tmp` content; blanket lowercase
`run` replacement is forbidden. The Task's PR #1408 attribution differs from this
base's local log (`#1408` is release recovery). Use actual files/commits as technical
evidence; the supplied preservation requirement remains binding.

### Preservation risks resolved by inspection/probe

* `session_events` captured rows select payloads by receipt/artifact key; Session
  identity and native IDs already have SQLite owners. Keep keys, event sequences,
  hashes and historical `run_` strings unchanged. Directory rename does not require
  reassigning history or recomputing usage attribution.
* The recorder reopens `events.jsonl` by pathname. A local temporary-directory
  probe showed an open file follows a moved inode, but pathname reopen fails and
  directory recreation splits old/new roots. Atomic rename alone is unsafe with
  old writers. Symlinks retain the retired interface and do not solve authority.
* `PromotionLock` serializes installers only. `startup_selection_during_switch`
  intentionally returns the prior selection until activation, including during
  candidate recovery. Neither that lock nor a switch receipt fences old writers.
  A new lock added to new binaries cannot retroactively constrain released ones.
* Launch locks, provider-client receipts, watch paths and persisted native endpoint
  references may embed the old root. Quiescence must include surviving providers,
  detached clients, open sequencers and scheduled launch sources, not just driver
  exit. Missing process evidence is unknown, never permission to move payloads.
* Native resume has a separate environment path. Updating CaptureHandle alone
  would leave resumed agent commands with stale identities.
* `tests/e2e/install_bootstrap.py` currently references `DAEMON` without a definition
  in the inspected release-packaging block. Reuse the isolation pattern, not an
  unverified assumption that the entire old harness passes. Repair only the needed
  CLI candidate fixture in this checkout; coordinate installation changes at its
  interface with LOO-292.

## Chosen architecture

Keep SQLite as the sole owner of AgentSession/FlowSession/Exec facts. Use
`captures/<two-key-characters>/<artifact-key>/` for Session capture payloads. This
names the bytes accurately and retains shard depth and opaque keys. Do not regroup
by Session ID: one Session has many captured inputs, and moving each input adds
migration states without changing its owner. Payload files remain history of their
captured event, not a new lifecycle record.

Use a narrow serialized capture context (key only; derive directory from the resolved
Home) where tooling genuinely needs the selected input. Proposed name:
`LF_CAPTURE_KEY`. Resolve it against SQLite's captured event and owning Session;
validate the manifest when payload access needs it. Do not create `LF_SESSION_ID`
by relabeling an artifact key. Existing typed AgentCaller/current Exec supplies
Session identity and causal provenance. Expose a small resolved caller accessor
from the journal context, since startup consumes the environment. Direct child
Execs inherit parent Exec ancestry; that does not independently bind Task work or
confer review/Flow control. Explicit Work/checkout binding keeps its existing owner.

Ready keeps its implicit actor target using resolved caller identity. Rename and
complete keep their explicit Session ID arguments. Expose that real ID to authored
agent instructions from the existing AgentCaller context; do not add another
environment identity merely to make a shell example shorter. Keep explicit historical artifact selectors for history/replay; mutation rejects
stale capture or replaced driver even when it belongs to the same conversation.
Malformed/foreign capture context cannot silently turn an agent comment into a
person's steer or a nested contribution into a top-level checkpoint.

### Offline, recoverable layout conversion

**Proposed deployment requirement for design review:** one controlled offline
maintenance window for the capture-layout transition. Old writers finish naturally;
this Task does not authorize killing them, changing schedules or installing code.
Before the window, the published CLI continues using its single existing layout.
The candidate never falls back to or dual-writes `runs/` during ordinary operation.
A candidate seeing an old-only populated layout reports conversion required without
creating `captures/`. New empty disposable Homes use `captures/` immediately.

The authorized installer owns conversion, using its existing pinned candidate and
switch recovery receipt. Ordinary history/status/replay reads never initiate it.
The window must exclude old CLI/app/provider launch sources at the OS/installation
boundary, including retained executable invocations. This cannot be proved merely
by checking for no current PIDs. In disposable acceptance the isolated account's
launcher is controlled. For the configured machine, an explicit operational freeze
and proof of writer quiescence are required before conversion is authorized. If
that cannot be established, stop before mutation; do not silently substitute a live
migration or call a fixture proof installed acceptance.

1. With external launch admission stopped, inspect all exact selected-Home Exec,
   provider and client evidence. Any live writer, unresolved process identity or
   active sequencer retains the old layout and blocks conversion with the owner
   named. A bounded check returns; it never waits indefinitely or signals a process.
2. Preserve a consistent SQLite backup, complete payload tree and matching prior
   executable after quiescence. Record source/target, candidate identity and backup
   in the existing switch recovery domain. No secrets or payload content in logs.
   Validate captures/references before mutation; missing historical payload remains
   explicitly unavailable, not fabricated or pruned. Unknown files move intact.
3. Fsync prepared conversion evidence, rename the complete root within the Home
   (`runs` → `captures`), fsync its parent. No copy/delete merge and no symlink.
   Update only operational absolute references that must route to moved bytes;
   preserve immutable historical strings and relative artifact references verbatim.
   Enumerate those references during implementation before enabling conversion.
4. Record layout completion only after payload/reference validation and database
   integrity checks. Gate candidate activation and launch reopening on that receipt.
   Recovery remains with the pinned candidate after the first mutation; old CLI
   fallback must not reopen writers against the converted Home. This is a required
   change to current switch behavior at this boundary, not an existing guarantee.
5. Retry recognizes old-only/prepared, new-only/moved and completed states from
   recorded evidence. Crash after rename but before completion finishes validation
   forward. Both roots, unexpected target content, mismatched backup/identity, or
   changed manifests stop without overwriting either side. No timestamp arbitration.
   Before mutation cancel safely to prior operation. After mutation only candidate
   recovery can finish; restoration of the full frozen pair is an explicit offline
   recovery operation, never an automatic downgrade while writers are live.

Extend the existing receipt only with facts necessary for recovery; a small layout
version is storage metadata, not another Run or attempt table. If schema changes
are needed, one Task draft is edited in place and materialized at release. Released
SQL, receipts and historical payloads are never rewritten to satisfy vocabulary.

## Alternatives and review pressure

* Rename only variables: rejected; leaves presence-based policy and mixed IDs.
* Rename live tree or use a permanent symlink: rejected by the writer probe and
  one-layout requirement. Copy-on-read/dual readers add indefinite split ownership.
* Move each capture beneath its Session: rejected; more states and key joins with
  no user benefit. Stable capture keys already identify subordinate history.
* Add a general execution-context/attempt registry: rejected; existing journal,
  AgentCaller and captured events already own the facts.
* Online migration with a new advisory lock: blocked for released writers that do
  not acquire it. Solving only new-code concurrency does not solve this Task.

Success means Jack can resume saved work without learning a migration identity.
Failure would be a cosmetically clean source tree that strands a pending review,
loses late recorder writes or downgrades into the old layout after partial conversion.
The review must judge the offline boundary explicitly; cleanup authorization does
not manufacture acceptance of that operational requirement.

## Implementation sequence (one coherent end state)

1. **This slice: remove runtime identity coupling.** Delete definition-only Run
   helpers/events/errors, resolve capture vs Session vs Trace/Exec names, replace
   presence checks and cut fresh/native-resume/review paths together. Keep the old
   on-disk layout until the conversion integration is complete; do not publish this
   as the finished Task. Focused proof: nested/direct checkpoint behavior, caller
   ancestry, provenance and stale-review rejection in `session_cli_tests`,
   `task_pr_authority_tests` and existing journal tests.
2. Implement the offline conversion/recovery at the installation boundary; cut
   capture paths, watcher/process readers, replay, ablation tooling and mutable
   reference resolution together. Prove old active writer refusal before deletion
   of old runtime path readers. There is one final path, no mixed-mode deployment.
3. Align Swift names/fixtures, current docs/skills/config and error guidance. Delete
   duplicated readers and superseded tests only after preserving their behavioral
   counterexamples in the remaining owner tests. Keep final retention inventory.
4. Gate the complete tree once. Measure production additions/deletions separately
   from tests/docs and file moves against this base; do not count renamed lines as
   architectural deletion. No release or installed conversion is part of delivery
   authorization supplied here.

## Done when / headless acceptance

Proposed new test names below are implementation targets, not executed checks.
All source invocations use disposable Homes, cleared inherited authority and pinned
candidate binaries as TESTING.md specifies. Installation tests require isolated OS
accounts/containers without host Home or credential mounts; HOME override is not
isolation. Stubs supply provider/network side effects; assertions inspect persisted
history and public results, not mock-call wiring.

| Scenario | Command for gate | Required observation |
| --- | --- | --- |
| Released populated Home, interruption, repeat and active writer | `uv run python scripts/test_task_installation.py --test capture_layout_preserves_released_sessions` (extend existing runner/registration) | Released fixture generated by checksum-verified prior CLI; same Session/native IDs, Task/PR membership, feedback, event order, usage and replay assets; root hashes preserved; active writer and launch-race case mutate nothing; retry after each conversion boundary finishes once. |
| Conversation + nested/direct commands + review | `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --test session_cli_tests --test session_lifecycle_tests --test task_pr_authority_tests --test flow_tests --test active_runs_watch --no-fail-fast` | Public resume/ready/complete and monitor/usage reads cross SQLite and new payload paths; exact generations reject old callers; no extra Task/Flow advancement; watch sees new evidence. |
| Runtime focused invariants | `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --lib --no-fail-fast` | One captured owner, direct/agent parent edges, checkpoint composition, missing/invalid context and progress-marker behavior preserved. |
| Python/tooling | `uv run pytest python/tests/` | Current ablation/dev/materialization callers use current commands/layout; retained historical cohort proof still reads its frozen input. |
| Swift consumers | `swift test --package-path swift` plus affected headless app build per TESTING.md | DTO fixtures, Task history projection and environment isolation agree. No display or permission prompt. |
| Static/history contract | `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `uv run python scripts/check_architecture.py`; `uv run python scripts/check_migrations.py` | Formatting/static analysis and immutable migration frontier checks pass; current instruction examples resolve. |

Conversion fault matrix also includes permission failure before rename, target
collision, malformed manifest, missing payload, stale PID reuse, detached provider,
old writer reopening after the admission check, post-rename interruption,
post-reference-update interruption, repeated finalization, and attempted old-binary
startup before recovery settles. Failure reports paths/owners and preserves bytes.
A test that only observes an open descriptor after rename misses the actual bug.
Unavailable container/macOS checks retain explicit CI ownership; actual failures
require repair. Final installed acceptance remains separately unproven until an
authorized published candidate is exercised on the configured machine.

## Intentionally retained references and measurements

Final inventory must explain each retained family with paths: immutable released
SQL and historical migration fixtures; historical `run_` artifact keys and receipt
fields; captured transcripts/archived chapter evidence; GitHub workflow/check runs;
release-run operation names and its release accounting; gate/check execution receipt
identities; ordinary verbs (`cargo run`, process run, `RunAtLoad`). Old context names
may be scrubbed at historical launch boundaries but never read to decide behavior.
Current errors, live DTOs and current user instructions get no obsolete aliases.

Kickoff production diff: **+0 / −0**; no implementation or runtime acceptance yet.
The final count must report Rust/Swift/Python/scripts production separately, excluding
builtin prose, tests/fixtures, historical SQL and generated outputs, and name its base.

Check: temporary-directory rename/reopen probe — reproduced missing pathname reopen
and two-root split; source inventory/review only, no runtime or installed proof.

## Adjacent interfaces

Read-only `lf task status` observations (snapshot 1791000703) show LOO-285 retained
release accounting with its managed Flow blocked at task sync, and LOO-292 waiting
at demo. Neither state authorizes intervention. Their checkouts remain untouched.
LOO-285 retains due-opportunity/outcome linkage, unknown-publication handling,
verification and two-settlement commitment; GitHub workflow run identity is valid
and must survive this cleanup. LOO-292 retains published installation/main-checkout
and configured macOS/wake proofs. This Task owns capture preservation scenarios and
must integrate through existing installation recovery APIs without claiming those
other Tasks' proofs or altering schedules. Read-only coordination does not establish integration acceptance or authorize
mutation of those Tasks. The Release child memory also records that operation
results must reach cron unchanged; returning a successful report of failure cannot
count as successful release execution.
