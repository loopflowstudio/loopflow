# Remove Ask

Implementation plan draft — 2026-10-01. Product direction established by Jack Heart; kickoff resolves the mechanisms below. No implementation has started.

## What to build

Remove Ask: headless agents fail normally when they lack what they need; the responsible Wave operator reads their logs, resolves what it can, and discusses necessary human input in its existing Wave chat and context.

Jack's intent:

> mostly headless agents should just fail and say they failed if they dont have what they need

> If human input is needed, the wave operator can do it in theri chat and in their context

> the failing agents dont need to do anything extra to communicate that they failed

The agent's normal output and recorded outcome are the evidence. No escalation command, extra report, notification, completion ceremony, or specially formatted handoff is required.

## Placement

Unresolved. No exact Wave was supplied; this design does not infer ownership from the checkout name.

## The demo

A Task's headless agent cannot proceed, explains why in its ordinary output, and ends unsuccessfully. The Task retains the failed Flow position and logs. No new conversation opens and no caller waits for an Ask answer. The Wave operator reads the failed work's existing status and logs. It fixes an authorized impediment or discusses the decision in the ongoing Wave chat. Once there is a reason to retry, it uses existing Task controls. The same Task and Flow continue without advancing the failed position prematurely.

The same failure behavior applies to taskless work; absence of a Wave never causes creation of an operator or conversation. Its caller receives the failure.

## Current system

- `lf/commands/ask.rs` creates a requested Session and waits for its completion through `ops/human_session.rs`.
- `lf/commands/flow.rs` intercepts a Blocked decision, launches `flow_unblock`, waits, and consumes Ask feedback through `store/sqlite/flows.rs::answer_flow_blocker`.
- `controller/task/mod.rs` opens Ask on decision failure and recovery. `ops/task.rs` can require completed Ask feedback before retrying.
- `ops/task_execution.rs` exposes failures already, but its recovery guidance directs people to unblock Sessions.
- `engine/builtins/wave/skill/wave_session.md` already reads Task status and evidence, but tells the operator to send human decisions to separate Sessions. `wave_operate.md` already inspects failed Task outcomes.

Paths above are relative to `rust/loopflow/src/`.

## Data structures and key functions

Keep `AgentSession` for ordinary durable conversations and native history. Keep `FlowSession`, its recorded failure, selected completion, cursor, and claim ownership. Do not add an escalation record or failure inbox.

Use existing `fail_flow` to persist failure and release execution through the normal driver path. Keep `retry_flow(&self, id: &str, direction: Option<&str>) -> StoreResult<FlowSession>` for explicit recovery at the same position. Remove the dependency on an Ask ID or completed Ask summary. Task status and logs remain the operator's evidence sources; no separate agent-authored summary is mandatory. Preserve explicit `--retry`; Task recovery with `--reason` passes that direction into `retry_flow` rather than obtaining feedback from an Ask. Plain reconciliation does not retry a known failure. Existing authorized Task steering remains available without becoming a required agent reporting step.

Remove `SessionKind::Ask` from runtime and wire models. One Task migration draft converts existing `agent_sessions.kind='ask'` rows to `conversation`, preserving identity, native history, request, completion facts, and work attribution. Leave `primary_scope` unset: these conversations must not displace the Wave chat. Clear the retired `unblock` skill on converted rows; never run its saved Ask completion protocol. Retain the nullable `request` column as historical evidence, with no new Ask writer; released schema constraints and historical `ask_exchanges` tables need no destructive rebuild. Preserve released migrations and immutable Session events. Old Ask completion must confer no Flow settlement authority.

### Concrete risk findings and choices

- **Shared launcher:** `ops/human_session/primary.rs::start` invokes `serve-ask`, `ask_exec_is_running`, and `ask_background_name` for primary conversations. Rename this surviving path to `serve-conversation`, `conversation_exec_is_running`, and `conversation_background_name`, updating every caller in the same cut. Keep the existing primary terminal naming behavior so a live chat is not duplicated. Rename `ask_launch_args` to `conversation_launch_args`; retain shared lock, prepared-input, native-resume, and provider-generation ownership. Do not delete these functions solely because of their names.
- **Blocked already means stop:** `engine/transitions.rs::finish_step` returns `FlowTransition::Blocked` without moving the cursor. The driver already records `FlowOutcome::Blocked` through `fail_flow` and returns an error. Delete its Ask interception so that existing path executes. Remove the false answer-return promise in `engine/flow_output.rs::instructions`; retain the existing decision output schema.
- **Failure evidence is already persisted:** `flow_sessions.failure_json` and the Session event history own outcome and evidence; Task execution status and Desktop derive from them. Preserve ordinary provider errors and bounded malformed-output correction. Unknown native completion remains unknown. No prose classifier or new failure-report command is introduced. An ordinary final answer saying work could not proceed remains available to the operator even when the provider reports a successful turn; this removal does not redefine provider success.
- **Upgrade is a data conversion:** the released schema permits `conversation` and preserves request separately from native history. Convert rows before deleting enum decoding. Test completed, open-native, and reserved-but-unlaunched Asks. A converted unlaunched conversation opens only through explicit ordinary Session action, using retained request context without the deleted `unblock` skill or Ask instructions. No migration launches a process or marks an unfinished conversation completed.
- **No automatic historical answer consumption:** a saved blocked verdict reaches normal failure when its driver next runs. Already recorded failures stay stopped. Completed former Asks remain readable evidence; they never automatically release a Flow. No cross-version live driver handoff is promised by this removal.

Deleting only the CLI would leave automatic Ask creation in controllers. Replacing Ask with an operator notification queue would recreate the reporting machinery Jack rejected. Removing the interception and retaining normal failure is the chosen approach. Success means the Wave operator can understand failed work from its existing logs; the main regression to prevent is silent retry or advancement after deleting the wait.

## Delete — do not maintain

- `lf/commands/ask.rs`, `AskArgs`, `SessionCommand::Ask`, and Ask dispatch/help.
- Ask reservation, keyed identity, launch, polling and feedback functions in `ops/human_session.rs`: `ask`, `ask_once`, `reserve_ask`, `launch_keyed_ask`, `flow_unblock`, `task_unblock`, `task_waiting_unblock`, `exec_ask`, `wait_for_ask`, `ask_message`, their key helpers, and `HumanSessionToken::Ask`. Preserve and rename the shared launcher functions identified above.
- `answer_flow_blocker`, Ask-backed controller recovery, and the Ask-feedback prerequisite in Task recovery.
- Ask kinds/actions in Swift Session models and fixtures; Ask-only tests in `human_session.rs`, controller tests, `session_cli_tests.rs`, `session_lifecycle_tests.rs`, and `SessionsStoreTests.swift`. Preserve shared conversation, history, claim, and review coverage.
- Ask exceptions in `store/sqlite/sessions.rs::summary_query` and `ready_session`; retain independent membership evidence and review readiness. Delete `store/sqlite/flows.rs::blocked_feedback_retains_conversation_and_pass_and_consumes_once`; replace it with failure/retry behavior coverage, not a new feedback mechanism. Update shared DTO fixtures under `tests/fixtures/dto/` and their Rust/Swift readers together.
- Ask-only `unblock` skill and all automatic callers. Remove Ask instructions from builtin surfaces, skills, docs, and generated goldens; preserve useful diagnostic guidance in the Wave operator skills.

## Forbidden outcomes

No renamed Ask, replacement queue, automatic child chat, polling waiter, agent reporting command, or Wave escalation prerequisite. No automatic retry on unchanged evidence. A normal successful provider turn must not become failure merely because its prose mentions difficulty; retain the existing execution/Flow outcome contract. Reading logs grants no new process or delivery authority.

## Internal slices

An indivisible removal, implemented in internal slices and shipped as one PR.

1. **This slice:** remove Ask interception, `answer_flow_blocker`, controller auto-launches, and Task recovery prerequisites together. Return Blocked through the existing transition; preserve Task identity, cursor, claims and retry semantics. Update recovery guidance in `ops/task_execution.rs`. Focused proof: `cargo test -p loopflow --test session_lifecycle_tests failure_without_ask` (new cases) exercises Task and taskless failure, recovery, and explicit retry with a simulated provider.
2. Delete Ask CLI/runtime/UI representations and exclusive tests, with the migration and shared-launcher rename in the same cut. Reject both `lf ask` and `lf session ask`; do not leave aliases. Preserve ordinary conversation launch, primary uniqueness, and Task review behavior. Prove populated-store conversion and shared Session DTOs.
3. Update `wave_session.md`, `wave_operate.md`, builtin headless/Loopflow guidance, `loop-decide`, and affected skills/docs/goldens. Operators inspect existing logs and bring unresolved judgment into Wave chat; authored Task reviews still use their own Sessions. Delete the `unblock` skill and obsolete references, including prompt instructions in `flow_output.rs`. Verify the full cut.

## Done when

Headless acceptance proves missing-input failure returns promptly, records the reason, spawns no Ask, survives driver recovery without spawning one, and retries at the same position through existing controls. Assert retained historical conversations after migration and no premature Flow advancement. Task sessions and explicitly authored human reviews still open, resume, and return feedback through their existing completion contract.

The new `failure_without_ask` lifecycle cases run a simulated provider through the public CLI in a disposable Home: emit a blocked decision or provider failure, assert nonzero foreground exit, inspect SQLite and `lf task status --json`, verify no extra conversation or cursor movement, reconcile twice without relaunch, then explicitly retry and reach the retained Task review. Read the original reason from Session history. A deadline catches accidental indefinite waiting; there is no provider/network latency target.

Gate commands:

```sh
uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build -p loopflow --bin lf
scripts/test_desktop.sh
uv run python scripts/check_architecture.py
```

Rust coverage includes the new lifecycle cases, migration fixtures, primary launch/reopen, CLI rejection, and retained review feedback. Desktop model/store tests consume updated Session and failed-Task fixtures and retain Task review actions. Native display diagnostics are not prerequisites. Implement runs focused affected tests; gate owns broader acceptance once.

## Accepted scope boundary

On 2026-10-01, Jack clarified: “no, Task sessions stay”. Preserve Task sessions, including explicitly authored `human: true` Flow reviews, their feedback, and their navigation contract. Remove only Ask and its automatic escalation conversations. Wave chat handles judgment needed to resolve failed work; it does not replace Task review sessions or complete them on Jack's behalf.

Jack values Task conversations as a private room that takes load off the Wave thread. Persistent versus one-off Task sessions remains open and is excluded from this removal. No new operator scheduler, notification channel, or delivery authority is included. Missing logs or failed status reads remain missing evidence, not proof that work succeeded or permission to retry.

Check: 2026-10-01 — source inspection and `git diff --check` passed; implementation/runtime acceptance remains for implement and gate.
