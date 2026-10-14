# Session entry map and acceptance coverage

Baseline diagnosis: `cf754f02cd486990f8e88d9a22829a8ffd600eba`, read 2026-10-14; entry paths and coverage updated during implementation. Companion to [the design](claude-permissions.md), not a separate design.

## Entry points

Paths below are relative to `rust/loopflow/src/` unless stated otherwise.
These paths implement the accepted [entry API migration](session-api-simplification.md).
Selection is shared; preparation is explicit and independent of output format.

| Entry | Current path | Required shared behavior |
|---|---|---|
| Interactive inline prompt or skill | `lf/commands/run.rs::build_prompt_at` → `run_prompt` → `lf/commands/util.rs::launch_session` → `agent::build_session_command` | Preserve the complete resolved config through native startup |
| Headless inline prompt or skill | `run_prompt` → `run_headless_prompt` → `agent::run_agent` → provider harness | Same common options; stream protocol remains provider-owned |
| Interactive saved-session resume/connect | `lf/commands/session.rs` → `human_session::select_connection` → `human_session::open` → `util::resume_session_with_env` → `agent::resolve_session_config` → shared command boundary | Current settings at saved checkout; saved native identity/account |
| Prepared Session with no native history | `human_session::open_waiting` → `serve_locked` → shared current-settings resolver → child `lf -i` via `conversation_launch_args` | Rejoins ordinary fresh launch; no new provider-option builder |
| Prepared Session with native history | `open_waiting` → `resume_native_session` → `resume_session_with_env` | Rejoins ordinary native resume |
| Task conversation API | `connect --task --prepare` → target resolver → `primary::ensure_task` → existing/new Session; connect follows rows above | Selection/preparation does not itself spawn a provider |
| Repository/Wave primary or new conversation | `connect --wave` / `session new` → `primary::ensure` / `replace` → prepared Session → ordinary connect | Scope owns checkout/association, not provider flags |
| Desktop Task conversation | `RegistryQuery::prepareTaskSession` → `connect --task --prepare --json`; `prepareSession` → `connect ID --prepare --json`; returned `open_argv` opens the terminal | Desktop does not resolve permission policy; explicit preparation launches nothing |
| Headless continuation | `lf -b connect ID --message TEXT` / contextual `resume --message TEXT` → `run.rs::resume` → `build_prompt_at` in saved cwd → `run_prompt` | Current settings; retain native history; do not replay the initial skill |
| Task execution / Flow skill | Task Workflow selects Flow; `lf/commands/flow.rs` launches child `lf` commands through `spawn` and `run_skill` | Descendant options survive CLI forwarding and arrive at ordinary headless launch |
| Live Codex connection | `human_session::connect_live_codex` → relay → `resume_session_with_env` with `NativeConnection` | Attach to existing provider; preserve ownership/token fencing; no startup-policy rewrite |
| Explicit historical replay | `lf/commands/replay.rs::replay_at` → captured `AgentProcessRequest` → `run_agent` | Captured inputs, deliberately distinct from current-settings resume |
| Internal direct agent calls | `ops/pr.rs`, `ops/pr_landing.rs`, `ops/commit.rs` → `run_agent`; terminal branch uses `agent::build_session_command` | Same provider translation; preserve caller's explicit policy |
| Harness recovery | Persistent Claude stream respawn uses `agent::build_session_command` with `ProviderInvocation::ClaudeStream` with saved native token | Reuse already resolved launch policy for process recovery within the same run |

Machine dispatch is an outer routing layer: resolution belongs at the destination checkout. Test forwarding of relevant options; do not make the local machine's defaults authoritative remotely.

## Test matrix

Cover Claude, Codex, and OpenCode where supported. Use two dimensions rather than multiplying every unrelated flag into a huge Cartesian suite: complete option coverage at the provider boundary, plus entry-point tests proving each caller supplies the complete config. Add explicit interaction cases where precedence or transport changes semantics.

| Group | Cases across fresh launch and resume |
|---|---|
| Provider/model | Configured default, explicit `--agent`, omitted model, persisted provider identity, supported resume model selection, inherited agent override |
| Permissions | `--yolo`; global/repo precedence; repo false overriding global true; vendor `auto` versus bypass; no YOLO; changed settings between stop/resume |
| Execution scope | Main checkout, linked worktree and git metadata roots, required Task boundary, boundary plus YOLO, no unintended writable roots |
| Claude options | Chrome on/off/inherited, turn limit where supported, response schema where supported, stream modes, explicit absence on unsupported surfaces |
| Input/context | System prompt file, saved hooks, native/translated skill, plugin flags, user prompt, clipboard/docs/diff contribution, paths with spaces, leading `---`, `--` placement and variadic `--add-dir` |
| Continuation | Fresh session ID versus resume token, no repeated seed/skill on resume, missing optional capture, history preserved, updated current settings |
| Account/environment | `--account`, `--only-account`, isolate/shared, saved account on resume, provider home, scoped overrides, control binary, attachment identity, no credential output |
| Session metadata | Native title, current participant context, Task/Wave binding, selected cwd, capture and descendant attribution |
| Transport | Terminal, stream, native connect, retry/respawn, explicit replay; expected protocol-only differences |
| Forwarding | Task/Flow children and machine dispatch retain declared options; context/explain previews launch nothing |

Build fixtures with the candidate binary pinned inside the network boundary. Environment-mutating fixtures share `loopflow_test_support::env_lock`; native fixtures use `TestLedgerGuard`, and blocking workers use `journal::with_test_ledger`. Capture arguments at the provider process boundary, not mocked internal calls. Isolate account/network/UI side effects. Assertions must not call the same production resolver to compute expected results.

## Coverage ownership

`tests/session_launch_contract.rs` records real argv, protocol requests and selected
non-secret environment. It covers fresh/resumed terminals for all providers,
headless continuations for all providers, current policy/model changes, global/repo
precedence, CLI overrides, Task preparation/connect (the Desktop command path),
Flow forwarding, context/identity, linked Git worktrees, account selection and
isolated-home preservation, and effect-free previews.

`agent::session::tests` covers stream turn limits/schema and independently rejects
turn-limit-only and schema-only terminal launch/resume for all three providers.
The CLI fixture proves schema-only rejection on fresh launch and native connect,
without spawning a provider or replacing the saved Session. Turn limits have no
CLI flag; their rejection belongs at the shared command boundary. The same suite
covers provider-only write scope plus YOLO across terminal/resume/stream boundaries.
`agent::tests::managed_task_scope_carries_exact_delivery_capabilities` separately
checks managed execution options; neither fixture proves OS write enforcement.
Existing `agent`, `harness`,
`lf::commands::util`, `lf::commands::replay`, `session_record`, and
`ops::human_session` tests retain skill/plugin input, large contexts, hooks,
structured output capture, historical replay, retry/respawn, remote attachment,
process fencing and lifecycle coverage. The direct native Codex `agent_tests` fixture
checks account flags before native client syntax, current policy in protocol
requests and literal prompt delivery outside argv. Existing remote routing tests
cover account restrictions and literal argument boundaries. The machine contract
fixture runs the candidate binary at both ends of simulated SSH, with separate
homes/checkouts and conflicting defaults. It asserts destination model, YOLO and
Chrome policy, explicit overrides on launch/connect, native identity and literal
prompt preservation, and no local provider launch.
`machine_commands::remote_session_open_returns_an_owner_pinned_terminal_command`
proves remote connect/resume preparation returns the same executable SSH command
in text and JSON, preserving literal arguments and stdin.

`task_flow_launch_tests::explicit_launch_policy_survives_task_workflow_nested_flows_and_provider_children`
records provider argv/account through direct Task binding, Task Workflow execution,
nested Flows and provider-issued children. Its six launches must retain explicit
model, Chrome, bypass and account selection. Task conversation preparation/connect
remains a separate entry from Workflow execution.

`harness::claude::tests::interrupted_capture_retains_policy_after_settings_change_and_fences_old_snapshots`
changes repo model, YOLO and Chrome settings after interrupting the first process.
Both process launches must retain the resolved policy; the replacement resumes
the native conversation and still fences stale attachment snapshots. Desktop
now requests explicit preparation and reuses its returned command. New
contract fixtures cover contextual selection, explicit Work targets, text/JSON
effect parity, current policy at the selected checkout, and primary succession
versus takeover. The main design retains one current verification line.

## Review findings

The original defect was the wide native launch signature dropping resolved
settings. Appending a single bypass flag would leave that failure class intact.
The implementation instead removes the duplicate builders and carrier. Review
also removed selective field-copying from headless resume, unified Codex policy
translation, kept live attachment outside current-policy resolution, and moved
direct-library account flags before the native argument separator. Replay tests
kept unsupported historical options reportable without blocking reproduction.
Coverage review pinned the simulated remote's provider executables ahead of
installed tools and removed timestamp-based selection ambiguity by resuming the
first Session before creating another. Claude stream account assertions use its
isolated provider home, preserving the existing transport contract. Compression
removed the remaining `ClaudeArgs` carrier and duplicate launcher fixtures.
Context-file precedence and blank-context coverage now use the shared command
boundary across terminal, resume and stream; linked-worktree CLI coverage also
checks that a literal prompt beginning with `---` travels outside argv: stdin
for Claude and protocol input for Codex. Native Codex tests retain configured
writable roots and inspect current policy in actual thread requests.
Post-#1552 native bootstrap resolves thread settings without sending a turn on
the bootstrap connection. The relay supplies pending input after the native
client subscribes. The failed-bootstrap contract proves the next explicit connect
retains the account and pending initial request while applying current settings.
