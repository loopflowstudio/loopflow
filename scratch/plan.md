# LOO-924: four launch kinds and a parsed agent choice

The launch design and implementation are in PR #1966. The remaining work is
gate and live demo. The concept review at `f07616744f:scratch/plan.md` records
the last implementation slice and all earlier decisions and checks.
`scratch/launch-states.html` preserves the original discussion and prior art.

Jack, 2026-11-17, quoting the review: "Two product ideas still lack one owner in
the code: whether a launch is managed, and which agent was chosen." / "this
seems worth fixing".

## Launch kinds

Jack: "my suggestions: work session, worker / chat, run", then "ok, batch is fine
for bare headless". On workers: "i guess wohat i would say is WOrker = (Manage &
Headless)" and "the worker run should definitely get the task steers".

| | Interactive | Headless |
|---|---|---|
| Managed, reports turns | `Run::WorkSession` | `Run::Worker` |
| Bare, no turn reports | `Run::Chat` | `Run::Batch` |

Transport selects `Run` before delegation. A held child takes up a work session.
A direct launch in the caller's terminal or pipe is a chat or batch, including
with `--task`, `--wave` or `LF_AS`. Managed launches report turns to the Machine
server, including the headless steps of Task Graphs and landing's `ci-fix`.
Discord Wave answers report turns as workers while the bridge waits for their
answer. Commit messages, PR text and replay use batches. `Run` stays in the
launch path, with no persisted launch type or process-list column.

Task membership supplies both usage attribution and steers regardless of launch
kind. The checkout, `--task` and `LF_AS` can supply membership. The Session's
`WorkSource` still records context provenance and helps select primary
conversations.

Jack: "resume connects to existing sessions". Resume reads the binding
(`bound_at`) and primary scope once before delegation. Chats reopen in their
terminal. Bound and primary conversations reopen as work sessions.
An unreadable store records a gap and falls back to a work session. A held child
keeps the parent's work-session choice. `runs_in_its_terminal` shares the same
classification. Fresh resume carries invocation settings and the inline-prompt
delimiter across exec.

`lf -b resume <ID> --message TEXT` remains a batch. A Task Graph's correction is
part of its worker step. Jack: "lf resume shouldnt take messages, i dont think?"
/ "that seems like a mixed API". LOO-958 owns Session command changes. The
headless → interactive demo is unavailable because `run_refusal` rejects the
recorded headless Session before transport classification.

## Agent choice

Config resolution parses the selection and headless fallback order before launch.
`AgentConfig.agent` carries a `HarnessKind` and optional model. Claude's configured
model folds into that selection. Native admission, missing-CLI diagnostics and
headless launch use the typed choice. Authored preferences, CLI inputs and
persistence keep strings at their boundaries. `chrome` and `codex_service_tier`
remain provider options. The parsing audit and retained call sites are in
[PR #1966](https://github.com/loopflowstudio/loopflow/pull/1966). Transcript methods
stay on `Harness` (Jack: "no opinion in this one"). `Launch.recording` holds its
store, Session and attachment.

## Delete — do not maintain

- `Holder`, `Place`, `transport::holder`, `WorkSource::is_managed`, the
  `named_work` delegation argument, `Launch.holder` and independent Work reads.
  `Run` owns launch classification and `Delegation::Here` carries it.
- `Run::Bare(Mode)`, `Run::Managed` and `Run::reports_turns`. The four launch
  names are the variants, and `Run::is_managed` supplies hook, thread naming
  and steering policy.
- Resume classification inside `delegate`, `SessionConnectionArgs::work_flag`
  and prepared-command work-flag forwarding. Both resume entry points use
  `session_run` before delegation and the selected Session ID owns identity.
- `provider_env_allowed_for_program`, `api_key_env_allowed_for_program` and
  `normalize_program_name`. Typed harness options own credential policy. Keep
  exported-key filtering and stored GitHub and provider environment behavior.
- Re-parsing the agent in launch consumers and `AgentConfig::resolved_agent`.
  Keep parsing where text enters.
- `PromptBuild.harness`, `.model`, `.mode` and the `prepared_config` argument
  to `begin_process_runtime` and `run_headless_prompt`. The typed agent and
  launch kind already carry those facts.

## Launch-name wording

Done. `Run` has the four variants, the delete list is gone from `rust/` and
`docs/`, and the glossary, `docs/dev/index.md`, `docs/dev/machines.md`,
`docs/sessions.md`, the transport README and the `transport.rs` resume comments
name launch kinds rather than places or lifetimes.

## Remaining checks

Gate owns the broader transport, configuration and Task performance checks.
Demo owns live steers and terminal lifetime.

1. Start `lf -b research '...'` in a Task checkout, then send a steer with
   `lf steer <TASK> 'note'`. The batch prints it after its next tool call,
   and the Task's usage includes the batch. A worker of that Task can read
   the same steer.
2. Quit `lf research` at a terminal and reopen it with `lf resume`. It runs
   in the same terminal. Close the viewer of `lf resume --task <TASK>`
   and reattach to the work session still running in its holder.

Earlier local checks are in `81925194af:scratch/plan.md`. The launch matrix now
specifies each launch kind's expected mode and turn reporting independently and
exercises all four harnesses against stand-in providers.

## Local checks, 2026-11-17

- `cargo build -p loopflow --bin lf` and `cargo fmt --all --check` passed.
- Focused `cargo nextest run -p loopflow -p loopflow-execution --lib --test integration` with the launch matrix, transport classification, launch policy, Claude and Pi hooks, credential and transcript checks, and documented commands passed all 23 tests. macOS linked with the previously observed compact-unwind warning.
- `uv run python scripts/check_architecture.py` and `uv run --extra test pytest -q tests/test_portable_architecture.py` from `website/` passed. The portable HTML was regenerated.
- `cargo clippy --all-targets -- -D warnings` and `git diff --check` passed.
- `cargo nextest run -p loopflow-execution --lib -E 'test(harness::agent_process) | test(transport::registry)'` passed all 31 after correcting the batch and bound-session fixtures and checking interrupted paste bytes at the child. The earlier paste failure passed in isolation before the fixture edit. That run predates merging main at `14149fac0c` (#1960, which touched `harness/server_connection.js` and `harness/testdata/pi_rpc.py` but no launch path); gate reruns the suites on the merged tree. Gate and live demo remain.
