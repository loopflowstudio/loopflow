# Finish the Run cutover — LOO-370

Design begun 2026-10-02; reconciled 2026-10-05. Jack Heart requested cleanup
while preserving conversations, history and work. This plan is not approval of an
installed-Home migration. Base: `a278d6bc1bd4373f78f27027b8ae249100ef14d3`.
Source implementation is authorized by the October 4 direction below.

## Current direction — October 4

Jack Heart authorized fully autonomous development and landing in the repository
conversation, without further interactive design/demo approvals. Jack suggested
`sessions/` as a possible replacement for `runs/`; this is a naming preference,
not approval of a particular layout or conflation of capture keys with Session IDs.
Resolve the layout against the actual owner, preserving retained identity and history.

The earlier design-review hold is superseded for source implementation.
Choose and prove the recoverable conversion strategy autonomously in isolated
released-Home fixtures. A maintenance boundary may be part of the designed deployment;
no interruption of live conversations, installed-Home conversion, installation,
schedule mutation or production release is authorized by this source-delivery request.
Retain remaining configured acceptance honestly after landing. The prior feature
Flow's human review gates are replaced by the autonomous implementation path at
Jack's request; preserve Task, branch, checkout and history.

## Source implementation status — October 5

The runtime slice uses `LF_CAPTURE_KEY` for subordinate history and the resolved
Session/Exec caller for ancestry and checkpoint composition. Native resume exports
its claimed driver's AgentCaller. Session mutations take durable IDs; history and
replay retain capture selectors. Public history DTO exports remain available under
`session_history`. Definition-only Run helpers and unused provider-fill wrappers
are deleted. Git owner fields name Trace/Exec while released JSON fields remain.
SQLite rename/readiness/completion and Task review settlement fence replaced
providers and stale captures inside their mutation transactions.

Compression removed rename's obsolete expected-capture argument: naming targets
the conversation, and the actor fence rejects stale providers/inputs. Review
prechecks reuse the storage fence rather than duplicate generation and capture
comparisons. Two controller fixtures exercised removed mutation aliases;
the repaired replacement-race fixture uses durable Session IDs and retains feedback,
client ownership, lock handoff and exactly-once completion assertions. Public CLI
coverage retains capture-key/prefix rejection and stale-actor mutation rejection.

Earlier runtime, controller, Session/authority/history and replay checks are
preserved at `71a784cdf5135fad8b26a21ddd6ea1ab354c4a93:scratch/finish-removing-the-retired-run.md`,
including the exact earlier checkpoints and the Session skip. Those focused passes
and formatting/Clippy results do not establish conversion, Desktop acceptance or
a full gate. October 5 builds supersede the earlier disk-capacity limitation.

Preservation findings remain binding: a native conversation can resume without its
old manifest; validate present payload without making absence erase SQLite identity.
The metadata-open fixture must drain output while waiting for provider exit: its
former pipe backpressure looked like a provider hang. File-backed output preserves
the bounded assertion.

Remaining indivisible source work:

- Implement offline conversion and candidate-owned recovery, including launch
  exclusion, frozen backup and operational-reference inventory. Installer
  advancement and recovery both have exact-SQLite shortcuts; neither proves
  filesystem conversion. Candidate recovery must become effective before the
  first layout mutation, rather than at later entry-gate activation.
- Move capture storage, watch classification, replay and ablation/tooling together
  to `captures/`. No payload root has moved. Keep the old layout until this cut
  can preserve populated history and interrupted conversion.
- Finish current-reference and native callback/Flow consumer audits, Desktop
  validation, populated released-Home fault matrix, final inventory/delta and gate.

The exclusion runner's pinned checksums were verified against published v0.13.3
SHA256SUMS. Docker was unavailable; no container or hosted result is recorded.
The executed macOS alias counterexample below remains contrary evidence. The expanded
`capture-exclusion.yml` probe also tests inode sealing and external storage; it
has no execution result. Its bounded fixture cannot establish arbitrary placement
coverage or production quiescence.

This runtime slice cannot land independently as the finished cutover. No installed
conversion, interruption, installation or production release is authorized.

## Deployment boundary

The selected source design uses an offline transition: conversations finish writing,
launch sources close, the frozen Home and executable are preserved, conversion
completes, then the same conversations reopen. Jack Heart's October 4 direction
permits designing and testing this boundary autonomously in isolation. It does not
authorize a configured interruption or installation. Duration remains unmeasured.

October 5 source inspection confirms that
`machine_install.rs::startup_selection_during_switch` still selects the prior binary
before activation, while `promotion_lock.rs` serializes installers only.
`advance_switch_store` skips the candidate at an exact SQLite frontier;
`recover_switch` can also finalize advancement without candidate migration work.
A layout receipt must participate in both paths. Candidate recovery and startup
exclusion must be effective before the first filesystem mutation; direct retained
binaries additionally require external launch exclusion. The privileged experiment
below tests pathname denial only. The retained-alias
counterexample invalidates it as the sole boundary; select a boundary covering
all access paths before implementing privileged targeting or recovery.

### Released writer bypass — October 5 source counterexample

Inspection of released tag `v0.13.3` at
`8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3` rules out treating an
installation-receipt version bump as the complete offline boundary:

- `bin/lf.rs::run` bypasses ordinary machine dispatch for installation, doctor
  and screenshot commands, then calls `journal::observe_process` before their
  command-specific checks. Direct retained binaries need not traverse the
  replaceable entry-gate executable.
- `journal::observe_process` opens `SqliteStore::open_existing_execs` and writes
  process evidence. That opener checks the Exec columns, not the installation
  receipt or migration frontier. An unknown receipt version or a new migration
  therefore cannot establish that the frozen database has no released writers.
- A SQLite write lock would exclude those writes only while its process lives.
  A converter crash releases it before recovery, so it cannot substitute for the
  persistent external launch exclusion required by this design.

This is source counterevidence, not an executed released-binary reproduction.
The conversion cut remains blocked on a concrete, crash-persistent offline
boundary. Its isolated proof must attempt direct released `install preflight`,
doctor and screenshot invocations both during conversion and after converter
death, checking the database as well as capture bytes. A test launcher that only
cooperatively declines new commands does not prove that boundary.

Before dependent implementation, specify the external owner that prevents the
original OS account from launching retained executables, how the pinned candidate
accesses that account's installation/Home while exclusion persists, and how
recovery retains exclusion after its own death. The existing installer resolves
the running OS account; running it as another account without explicit target
ownership would select the wrong installation. Source authorization permits
designing this mechanism in isolation, but supplies no installed maintenance
authority. No storage path, receipt version or schema is changed by this finding.

### Pathname exclusion rejected; inode sealing unproved — October 5

Full proposal/probe history is preserved at
`a7cb724ed3ae9166fb53808d288fc5aea3e06c2a:scratch/finish-removing-the-retired-run.md`.
The earlier whole-Home ownership proposal is at
`e4858f2e540055cca60a25544480495f0ba47c9a:scratch/finish-removing-the-retired-run.md`.

The executed unprivileged macOS `--probe-aliases` counterexample closes every
handle, denies traversal of a temporary Home, then opens outside hard links to
its payload and SQLite database. Both originals change; SQLite checkpoints WAL
through the alias. The original pathname is confirmed inaccessible. Permissions
are restored and only fixture files removed. This disproves directory exclusion
even with no surviving descriptor or process; it is not a privileged or
released-CLI reproduction.

Inode sealing instead changes file ownership as well as directory boundaries.
Hard links share that ownership; mode changes alone let the original owner
restore access through an alias. The authored Linux probe covers external storage
through a `runs` symlink, payload/database aliases, alias `chmod`, released
journaling through aliased `LF_HOME`, and external-directory replacement. Writes
must succeed before sealing and after restoration, with protected bytes unchanged
while sealed and after reader death. Known retained writers exit before snapshots;
the privileged reader uses immutable SQLite access to avoid creating its own WAL.

Docker cannot connect to its daemon; Linux execution remains CI-owned. The VM
manager was not started without a selector because that would resume unrelated
machines. No hosted result, populated conversion or privileged macOS proof exists.
The released store is empty and its payload synthetic.

The full-sealing fixture uses fixed roots and an in-memory restoration list.
Its privileged reader dies only after sealing finishes while the controller
survives. Even a pass would prove persistence of completed permissions, not
interrupted sealing or recovery after losing that list. The interruption probe
below retains the missing boundary as contrary evidence.

Production needs stable resolved Home/payload/native objects, link counts and
namespace ancestors, including external storage, ACLs, mounts, writable mappings
and inherited descriptors. Preserve supported placements and unrelated owners;
seizing a shared ancestor or rejecting the placement cannot supply exclusion.
Candidate recovery and durable restoration evidence must precede each metadata
mutation; seal before final quiescence and repeat quiescence on recovery, because
permissions cannot revoke handles. Keep the frozen backup, validated restoration
and populated fault matrix. No fixture helper establishes these obligations or
belongs in the installer as a proven boundary. Target-account implementation and
the layout cut remain dependent on a replacement exclusion design.

### Interrupted sealing and shared namespace — October 5 counterexample

`--probe-recovery` now fsyncs a fixture restoration receipt (path, device/inode,
owner and mode) before a separate worker seals one file and dies by SIGKILL,
leaving its directory unsealed. The executed unprivileged macOS probe replaces
that directory through its writable shared parent while unrelated writes continue.
Fresh-process recovery detects the changed inode before restoring any metadata;
both displaced history and replacement bytes remain intact. After the fixture
explicitly restores the namespace, two fresh recovery processes restore the
original metadata from the receipt. No in-memory restoration list crosses into
those processes. This is process-death evidence, not a power-loss proof.

The container variant seals the file as root and attempts replacement as its
fixture account; Docker remains unavailable, so this variant is unexecuted and
CI-owned. The existing full-sealing probe instead seizes the entire external
parent; its exclusion assertion cannot prove preservation of unrelated access.
A durable receipt prevents blind restoration onto replacement inodes but does
not establish exclusion during partial sealing. Dependent conversion remains
stopped: a replacement design must establish admission/namespace exclusion before
metadata changes without taking unrelated shared-directory access. Repeated
quiescence after interruption, ACL/mount preservation, interruption between every
ownership/mode mutation, and candidate-owned recovery remain unproved. The small
fixture recovery helper is not installer code and does not select a deployment
mechanism.

### Recovery check/write race — October 5 executed counterexample

The expanded `--probe-recovery` pauses a recovery process after it accepts every
saved inode identity. Another process replaces the directory and writes unrelated
content; recovery then changes the replacement file from mode 0600 to the saved
0644 and exits successfully. The displaced original remains sealed at 0400. Both
histories retain their bytes. Explicit pipe barriers establish the interleaving;
this is executed unprivileged macOS evidence, not a probabilistic stress result.

Four other cases interrupt at sealing owner/mode and restoration owner/mode
boundaries. After the fixture repairs the namespace, two fresh-process retries
restore metadata and retained bytes from the durable receipt. Portable ownership
stages perform no chown; the Linux variant performs it but remains CI-owned.
Restoration interruptions occur on the directory before the payload is restored;
these cases do not cover every inode, ACL, mount or power-loss boundary.

**Design disposition:** reject pathname identity prechecks followed by pathname
restoration as candidate recovery. They are not merely incomplete validation:
a successful return can modify unrelated replacement metadata. The helper remains
only a negative experiment. Descriptor-based mutation could avoid that particular
wrong-inode write, but would neither prevent namespace replacement nor exclude
new or retained writers; it is not selected as a conversion boundary.

Dependent conversion remains stopped under the implement skill's architectural
counterexample rule. No admission/namespace exclusion design is proved. Repeated
quiescence after interruption is also unproved: these fixtures own their children
and cannot discover production writers. A replacement must provide an external,
crash-persistent admission owner before touching metadata, preserve unrelated
shared-directory access, and keep candidate recovery effective through restart.
Further permission-helper refinements cannot stand in for selecting that owner.

## Problem and observable outcome

The original inventory found capture keys called Run IDs, presence-based execution
policy and Session payloads beneath `runs/`. The runtime slice removes that policy
coupling; payload storage and tooling still retain the old layout. The complete
cutover must prevent wrong-identity targeting and loss of live history.

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

**Selected source design; configured deployment not authorized:** one controlled offline
maintenance window for the capture-layout transition. Old writers finish naturally;
this Task does not authorize killing them, changing schedules or installing code.
Before the window, the published CLI continues using its single existing layout.
The candidate never falls back to or dual-writes `runs/` during ordinary operation.
A candidate seeing an old-only populated layout reports conversion required without
creating `captures/`. New empty disposable Homes use `captures/` immediately.

The authorized installer owns conversion, using its existing pinned candidate and
switch recovery receipt. Ordinary history/status/replay reads never initiate it.
The window must exclude old CLI/app/provider launch sources at the OS/installation
boundary, including retained executable invocations. Neither an empty PID sample
nor a controlled fixture launcher establishes this. The administrator-owned Home
proposal is insufficient because outside aliases remain writable. Target-account selection, recovery and restoration depend on a
replacement boundary. The sequence below remains conditional on that design and
proof. Isolated acceptance must attempt
direct released commands after converter death as well as during conversion.
Configured deployment additionally requires authorization for the operational
freeze; fixture success cannot establish installed acceptance.

1. Establish candidate recovery and durably record restoration evidence before
   any exclusion mutation, including ownership/ACL changes before the layout move.
   Under a proved external admission boundary, stabilize the resolved-object and
   namespace inventory, establish exclusion, then validate quiescence. Recovery
   must repeat that validation; permissions cannot revoke retained handles.
   Inspect all exact selected-Home Exec,
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
Isolated acceptance must prove the offline boundary; source-delivery authorization
does not establish configured launch exclusion or installed preservation.

## Delete — do not maintain

Completed cuts and their detailed rationale are preserved at
`71a784cdf5135fad8b26a21ddd6ea1ab354c4a93:scratch/finish-removing-the-retired-run.md`:
unused capture constructors/provider helpers, replay's duplicate selector and
manifest readers, request clones, mutation aliases and runner orchestration.
Retain `resolve_manifest` as replay's selector owner, `inherited_capture_key` for
subordinate history and `journal::agent_caller` for Session/Exec provenance.
Rename still needs its distinct managed-review lock/relookup path.

Direct-skill tests now use the shared `test_ambient::EnvGuard`; their private
`EnvironmentRestore`, manual identity clearing and temporary key vector are
removed. Save PATH before clearing it, and keep the guard inside the environment
lock. The shared guard also clears agent caller and Task authority for the paired
research fixture. This changes fixture isolation, not production behavior.

The exclusion fixture shares one retained-writer lifetime across pathname denial
and inode sealing. Each exit verifies the exact appended bytes: a suffix check
could mistake the first writer's text for the second writer's success. SQLite
count readers close explicitly before the next phase, and snapshots classify
entries from their existing `lstat` result. Linux execution remains with CI.

The exclusion probe now separates command dispatch from the released-writer
scenario, reuses one fsync helper and one bounded metadata-worker launcher.
Recovery still runs in fresh processes from the durable receipt; the full-sealing
probe retains its distinct in-memory metadata and unproved recovery boundary.

Still delete the `runs/` root reader in `session_record::record_dir`, hard-coded
watch classification and `context_ablation.py`'s old layout/`--runs` interface
together with recoverable conversion and surviving-path preservation tests.
Do not polish those predecessor paths or remove immutable keys/receipt fields.
The released-writer and retained-alias counterexamples above prevent that dependent cut.

## Remaining implementation sequence

The earlier iteration feedback to implement target-account selection next predates
`71a784cdf`'s alias counterexample. That implementation order is superseded:
exclusion design is unresolved, not merely awaiting a hosted check. Explicit
account targeting is conditional on the replacement design needing it; adding
privilege machinery alone cannot repair the demonstrated bypass. Jack Heart's
autonomous source authorization still permits selecting and proving a replacement
in isolation, without another product approval.

1. Replace the contradicted whole-account-Home boundary with an offline exclusion
   design covering all access paths, not only original pathnames. Retain both
   released-writer and newly opened alias counterexamples. Select and prove that
   boundary before dependent privilege/recovery implementation; no replacement is
   currently proved. The interrupted-sealing probe above now demonstrates the shared-namespace
   gap; a durable metadata receipt alone cannot close it.
   Then implement any required OS-account targeting, durable
   recovery/restoration, and candidate-owned conversion through exact-schema
   advancement and recovery. Inventory mutable absolute references, preserve the
   frozen SQLite/payload/executable pair and prove released writers stay excluded.
2. Cut storage, watcher classification, replay and ablation/tooling to `captures/`
   together; remove the old runtime layout reader only with recoverable conversion.
   `record_dir` still derives `runs/`, watch paths hard-code its depth/root, and
   `scripts/context_ablation.py` still exposes `--runs` and stages that layout.
3. Finish the cross-language/reference audit and native callback/Flow proofs.
   Desktop history names and environment scrub lists have changed; Desktop build
   and behavior remain unverified. Keep released receipt fields and historical
   artifact keys unchanged. The removed helpers, `SelectedCapture` projection and
   Session mutation repairs are completed runtime work, not conversion evidence.
4. Run the populated released-Home fault matrix and affected gate once on the
   complete tree. Record retained-reference paths and production additions/deletions
   against `a278d6bc1bd4373f78f27027b8ae249100ef14d3`, separating Rust/Swift/Python/
   scripts from tests, prose and moves. The current integration base is
   `8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3`; distinguish upstream changes from this
   Task's delta. No release or installed conversion is part of source delivery.

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

Production changes now exist in the working tree. Final delta and inventory remain
outstanding; file moves must not count as architectural deletion.
The final count must report Rust/Swift/Python/scripts production separately, excluding
builtin prose, tests/fixtures, historical SQL and generated outputs, and name its base.

## Adjacent interfaces — reconciled October 5

Infrastructure memory records Jack Heart's October 4 closure of LOO-292 on actual
installation and checkout evidence; the earlier waiting-at-demo snapshot is stale.
That acceptance does not cover this capture conversion. LOO-285's merged delivery
still lacks its two qualifying unattended settlements. Preserve due/outcome linkage,
unknown-publication handling and legitimate GitHub workflow run identities.

The only immediate child, Release, has been read in full (GOAL.md and MEMORY.md).
It records v0.13.2 publication/install, but no verified public receipt from manual
recovery. Upstream `e1ec32929` (#1441), included in this branch's base, removes the
retired UI receipt prerequisite and replaces removed `lf catalog` smoke with
`lf list --json`; source integration does not establish installed or scheduled
acceptance. Required headless and public-artifact checks remain. The conversion
fixture must use the existing candidate-owned recovery interface without claiming
these adjacent proofs or changing their schedules/checkouts.

Check (October 5, implement): `uv run python tests/e2e/capture_exclusion.py --probe-recovery` passed all five counterexample/retry cases; Ruff and `git diff --check` passed; privileged Linux cases remain CI-owned, not conversion acceptance.
