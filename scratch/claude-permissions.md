# Consistent Session launches

Accepted architecture, 2026-10-14. Jack Heart accepted the scope and resume policy below, then added the external Session API simplification. Shared provider launch consolidation is implemented; the accepted entry API migration is implemented. The coverage map records current launch paths and policy assertions. Standalone design; no Task or Wave allocation.

## What to build

Resolve launch settings once and apply them through shared provider adapters for every fresh or resumed Session, regardless of how the caller reaches it.

Jack's anchors: “but lf should be overriding it”; “comprehensive testing for all launch flags for both launch and resume paths”; “map out all the different ways to get into sessions (launch, resume, through task apis)” and “reduce the number of possible code paths”.

**Accepted decision, Jack Heart, 2026-10-14:** stopped-session resume uses current settings, including current YOLO. Conversation identity and account remain attached. Explicit historical replay remains reproduction of captured inputs.

This is an indivisible consolidation, implemented in internal slices and shipped as one PR. The keystone includes native launch, native resume, headless continuation, and Task/Flow entry coverage across Claude, Codex, and OpenCode. No follow-up Tasks are proposed.

## The demo

With Claude configured for `auto` and Loopflow configured for YOLO, launch a Claude Session through the CLI and through a Task's Desktop conversation. Both start in bypass mode. Stop one, change current settings, and resume it: its identity/history survive and the new process uses current settings. Connecting to a live process preserves that process and its existing settings.

## Implemented ownership

`agent/session.rs` owns the terminal/stream command boundary and current-settings
resume resolution. Fresh terminal callers pass `AgentConfig` intact. Headless
continuation retains the resolved config and adds only the newly gathered turn
content/environment. `AgentProcessRequest` still captures historical inputs;
its serialized shape is unchanged.

Claude terminal and stream launches share `claude_options`. Codex terminal and
headless launches share the app-server policy through `codex_permission_policy`.
Native Claude prompts use stdin; native Codex bootstrap resolves thread settings
and its relay delivers pending protocol input after the native client subscribes,
preserving upstream #1552. OpenCode terminal and server
launches share `session_environment`. Native account routing, attachment fencing,
hooks and process settlement retain their existing owners. Live Codex attachment
constructs connection syntax without resolving a new launch policy.

## Delete — do not maintain

Removed in this diff: independent `util::build_session_command` and
`build_resume_session_command`; wide native launch/resume option lists;
`AgentCapabilities` and its capture copies; `build_claude_stream_session_args`;
OpenCode's separate environment builder and `context::configure_opencode`.
Required behavior survives in the shared provider boundary and existing lifecycle
fixtures. Removed exact-array tests that encoded incomplete native commands;
retained metadata, environment, attachment and lifecycle coverage.

Also removed: `build_model_command`, `build_claude_command`,
`build_codex_command`, `build_opencode_command` wrappers and duplicate builder
tests. `build_session_command` dispatches directly to provider options; Codex's
terminal and protocol adapters share boundary resolution. OpenCode context setup
mutates the already parsed settings, removing its second environment reader and
JSON round trip. Assertions for provider defaults, context, metadata roots and
execution boundaries now exercise the shared path.

Review removed the direct runner's program/args repacking, impossible empty-command
check, repeated environment application and cwd lookup. Resume model selection
reuses the preflight history check, retaining refusal before existing-process
changes. Fixture Wave creation is shared; no production abstraction was added
for tests.

Removed `ClaudeArgs`, its model resolver and exclusive struct/argv tests;
`claude_options` translates `AgentConfig` directly using the already parsed
model. Context-file precedence and blank-context assertions exercise the shared
boundary. Removed `util`'s duplicate bare-model/worktree argv tests and exclusive
Git fixture; the linked-worktree contract retains literal-prompt transport
assertions. Resume constructs its execution boundary once after combining saved
and current Task scope. No remaining launch-consolidation deletion targets. The entry plan records the
shared CLI option shape and removal of obsolete remote account routing.

Post-sync compression removes `codex_launch_policy` and its always-true `auto`
switch: the surviving policy consumes `AgentConfig` directly. Native and headless
startup already share app-server transport; native approval handling remains in
the harness. The direct terminal runner no longer carries an unreachable Codex
account branch. Its stale Codex argv fixture now uses the shared native protocol
stand-in to prove account flag placement, policy and literal input delivery.
`build_session_command` now dispatches once per provider after selecting fresh,
resumed or stream input. Claude options, including its headless response schema,
stay in `claude_options`; provider transport and native identity stay in the
command builder. This removes repeated provider dispatch without changing
stdin/protocol delivery or adding a launcher abstraction.

## Constraints and forbidden outcomes

No schema change is expected. Do not add a provider registry, factory trait, parallel permission resolver, or compatibility launcher. Do not change Task/Workflow semantics, infer new process authority from membership, or restart live providers merely to apply settings. Missing optional historical prompt material must not prevent native resume.

No silent flag loss: every supported option has an explicit effect or a documented transport-specific disposition. Preserve deliberate execution boundaries; YOLO must not accidentally weaken them. User-level Claude configuration is not the product fix. Do not claim YOLO defeats vendor rules that apply even in bypass mode.

## Remaining work

The accepted [Session entry migration](session-api-simplification.md) now shares
contextual/explicit target resolution and preparation between CLI and Desktop.
The launch map records the surviving entry paths. Implementation, compression,
upstream conflict recovery and this reconciliation are complete. The recorded
focused checks apply; no implementation repair was needed during reconciliation.
Queue gate is complete under Jack Heart’s latest
[delivery boundary](delivery-boundary.md). Publication and landing remain outside
queue; the root conversation owns the authorized landing with waiting. No Wave memory applies to this unbound work. Caller-mode reduction and
Wave worktree delivery policy remain undecided and outside this implementation.

Review corrected prepared first-open model selection: the child launch now uses
the shared current-settings resolver instead of pinning the preparation-time
model. Current settings, native identity/account, live attachment and explicit
historical replay retain their separate semantics. No schema changes.

The 2026-10-14 upstream sync reached `1429903732` (#1552), then restoring
caller edits conflicted in six files. Those conflicts are reconciled; the original
stash `0a6333cee958aa15e3346e296d0f82529560f441` remains as recovery evidence. The sync sequencer had already completed
(`lf sync --continue` reports no sync in progress). Codex uses one protocol
startup for terminal and headless paths, with a separate passive native-client
command. The contract now observes protocol settings and stdin instead of the
removed prompt argv. Startup failure recovery and saved-account pinning survive.

Integration review removed JSON-only remote preparation: text and JSON now use
the same bounded request and return the same machine-owned SSH command, for
both connect and contextual resume. The existing remote-command regression
checks format parity and executes the returned command with literal input.

## Done when

`rust/loopflow/tests/session_launch_contract.rs` exercises real CLI entry points, disposable homes/checkouts, stand-in providers recording argv, relevant non-secret environment and input. Exercise all options in the linked matrix, both presence and absence. Assertions state expected provider behavior independently of production builders. No external network, display, real account, or permission dialog; stand-in servers use local transport.

Gate acceptance requires passing `cargo test -p loopflow --test session_launch_contract`, `cargo test -p loopflow --lib lf::commands`, `cargo test -p loopflow --lib agent::`, `cargo test -p loopflow --lib harness::`, and `cargo test -p loopflow --lib ops::human_session`, with inherited `LF_*` removed and stdin closed, plus `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`. The accepted Desktop preparation migration adds affected headless view/service tests. Entry API acceptance remains as specified in the linked plan, including selection, text/JSON effect parity and identity preservation.

The Task Workflow policy fixture is compiled into the library; gate includes
`cargo test -p loopflow --lib task_flow_launch_tests` for that changed suite.

The entry migration also affects the standalone `global_commands` and
`session_lifecycle_tests` binaries; integration `machine_commands`, Session
CLI, DTO and documented-command tests; and Desktop RegistryQuery, SessionsStore,
WorkDestination and TaskFlowProof tests. Gate runs their affected suites once.
Focused implementation runs use those actual test-binary names.

The 2026-10-14 changed-aware plan (`uv run python scripts/test.py --list`)
selects architecture, Rust inventory/format/clippy, materialized Rust tests,
isolated data-clone tests, website tests and headless Swift tests. Its Rust
selection includes the registered 15-test launch contract, startup recovery,
Session recording, provider, Task/Flow, CLI and DTO coverage above. Swift includes
RegistryQuery, SessionsStore, WorkDestination, TaskFlowProof and DTO fixtures.
Gate executed the selected commands with candidate pinning and inherited `LF_*`
clearing at the materializer/network boundary and closed Cargo stdin. The runner’s
resource inventory timed out before testing; its bounded phase runner executed
the same selected commands directly with 70 GiB free initially and 67 GiB at
completion. Initial failures remain in `.lf/tmp/gate/session-launch/`; repair and
completion evidence is in `.lf/tmp/gate/session-launch-repair/` and
`.lf/tmp/gate/session-launch-final/`. No cached passing result was reused.
Hosted UI judgment remains outside this headless gate; Linux-only reconnect and
shutdown behavior remains CI-owned.

## Gate review findings

Headless continuation with no saved launch context discarded the freshly gathered
workspace context. It now retains that context as a fallback; the existing saved
workspace/provenance regression proves the repair. Saved context still takes
precedence, and current settings, account identity and protocol prompt delivery
remain intact.

Gate also repaired stale fixtures: Claude harness configurations now name Claude;
resume assertions follow Codex protocol transport; the Wave command registry
classifies the new Session selectors; and the process-outcome proof resolves an
explicit missing Wave through resume. The development handoff fixture now requests
`--prepare` and restricts PATH to fixture/system tools. Its former JSON-only call
attempted the installed Codex client and failed before submitting input; the test
group was stopped, the fixture repaired, and unfinished checks resumed. No
production provider was used to satisfy a test. Recovery stash and comparison
HTML remain preserved. No delivery or Wave worktree policy changes were made.

## Measure

One common-option translator per provider, with every Session entry reaching it. Matrix coverage records every launch-relevant option and entry; tests must fail if permissions, Chrome, context, account routing, or boundaries are dropped.

Check (2026-10-14): `TMPDIR=/private/tmp uv run python scripts/test.py --reuse-passing < /dev/null` stopped at the resource-inventory timeout before product tests; the selected bounded command plan then passed after repairs: `check_architecture.py`, `check_rust_test_inventory.py`, `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, materialized/network-isolated `cargo nextest run --all --no-fail-fast --build-jobs 4 --test-threads 4 -E 'not test(data_clones::)'` plus focused repair/unfinished-test runs (2,587 distinct passing tests, including launch contract 15/15), separately materialized `cargo test -p loopflow --test integration --jobs 4 data_clones:: -- --test-threads 4` (4/4), `cd website && uv run python dev.py test` (75/75), `cargo build -p loopflow --bin lf --jobs 4`, `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone` (508/508), `check_swift_multiplatform_boundaries.py`, and `git diff --check`; Linux-only `planning_reconnect_tests` and Linux shutdown acceptance deferred to CI, ignored live-provider/installation proofs not claimed.
