# LOO-924: four launch kinds and a parsed agent choice

The launch design and implementation are in PR #1966. Nothing in the design
is left to build; gate remains. The concept review at `f07616744f:scratch/plan.md` records
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
| Managed, reports turns | `LaunchKind::WorkSession` | `LaunchKind::Worker` |
| Bare, no turn reports | `LaunchKind::Chat` | `LaunchKind::Batch` |

Transport selects `LaunchKind` before delegation. Only a work session goes to
the holder; `lf task run` detaching from its terminal is
`transport::detach_current`, not a launch kind. A held child takes up a work session.
A direct launch in the caller's terminal or pipe is a chat or batch, including
with `--task`, `--wave` or `LF_AS`. Managed launches report turns to the Machine
server, including the headless steps of Task Graphs and landing's `ci-fix`.
Discord Wave answers report turns as workers while the bridge waits for their
answer. Commit messages, PR text and replay use batches. `LaunchKind` stays in the
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
  `LaunchKind` owns launch classification and `Delegation::Here` carries it.
- `Run::Bare(Mode)`, `Run::Managed`, `Run::reports_turns` and the name
  `harness::Run`. The four launch names are `LaunchKind`'s variants, and
  `LaunchKind::is_managed` supplies hook, thread naming and steering policy.
- `delegate_current` and `LaunchKind::bare`: a direct command never delegates,
  so `LaunchInput::kind` selects its kind. `delegate` serves only resume.
- Resume classification inside `delegate`, `SessionConnectionArgs::work_flag`
  and prepared-command work-flag forwarding. Both resume entry points use
  `session_kind` before delegation and the selected Session ID owns identity.
- `provider_env_allowed_for_program`, `api_key_env_allowed_for_program` and
  `normalize_program_name`. Typed harness options own credential policy. Keep
  exported-key filtering and stored GitHub and provider environment behavior.
- Re-parsing the agent in launch consumers and `AgentConfig::resolved_agent`.
  Keep parsing where text enters.
- `PromptBuild.harness`, `.model`, `.mode` and the `prepared_config` argument
  to `begin_process_runtime` and `run_headless_prompt`. The typed agent and
  launch kind already carry those facts.

## Launch-name wording

Done. `LaunchKind` has the four variants, the delete list is gone from `rust/` and
`docs/`, and the glossary, `docs/dev/index.md`, `docs/dev/machines.md`,
`docs/sessions.md`, the transport README and the `transport.rs` resume comments
name launch kinds rather than places or lifetimes.

## Remaining checks

Concept review of `275a936bbb`: the delete list is gone from `rust/` and `docs/`,
and `Delegation::Here` carries `LaunchKind`. Realign found no upstream change
since merge-base `2593c489d7`.

Gate owns the broader transport, configuration and Task performance checks.
QA on 2026-11-17 drove both demo checks live with a stand-in Claude
(`scratch/qa.html`): a batch in a Task checkout received `lf steer` after its
tool call and lists under the Task in `lf usage`, a worker received the same
kind of steer, a chat reopened in its terminal, and a work session reattached to
its held provider after the viewer closed. Codex, OpenCode and Pi were not run live.

Earlier local checks are in `81925194af:scratch/plan.md`. The launch matrix now
specifies each launch kind's expected mode and turn reporting independently and
exercises all four harnesses against stand-in providers.

## Local checks, 2026-11-17

- After the second-review build: `cargo clippy --all-targets -- -D warnings` clean; `cargo nextest run -p loopflow -p loopflow-execution --lib --test integration -E 'test(transport::tests::) | test(harness::launch_matrix) | test(harness::context::) | test(context_block::) | test(run::launch_policy_tests::) | test(documented_commands::) | test(harness::native_session::) | test(human_session::)'` passed 105/105. No local test drives `lf task run`'s detach; gate's Task run fixtures cover it.

- `cargo build -p loopflow --bin lf` and `cargo fmt --all --check` passed.
- Focused `cargo nextest run -p loopflow -p loopflow-execution --lib --test integration` with the launch matrix, transport classification, launch policy, Claude and Pi hooks, credential and transcript checks, and documented commands passed all 23 tests. macOS linked with the previously observed compact-unwind warning.
- `uv run python scripts/check_architecture.py` and `uv run --extra test pytest -q tests/test_portable_architecture.py` from `website/` passed. The portable HTML was regenerated.
- `cargo clippy --all-targets -- -D warnings` and `git diff --check` passed.
- `cargo nextest run -p loopflow-execution --lib -E 'test(harness::agent_process) | test(transport::registry)'` passed all 31 after correcting the batch and bound-session fixtures and checking interrupted paste bytes at the child. The earlier paste failure passed in isolation before the fixture edit. That run predates merging main at `14149fac0c` (#1960, which touched `harness/server_connection.js` and `harness/testdata/pi_rpc.py` but no launch path); gate reruns the suites on the merged tree.
- Compress: `lf session list --on` and `lf session move -a` parse through `config::Agent::parse` (deleted `SessionOn`); the four `Capabilities` literals are rustfmt-formatted again. `cargo nextest run -p loopflow-execution --lib -E 'test(human_session::) | test(moves_bulk) | test(harness::tests)'` passed 48/48; execution clippy clean. No test exercises `--on`; the predicate is unchanged.

- Compress after `275a936bbb`: `delegate_current` could no longer delegate, so it became `LaunchInput::kind`; `detach_current` and `delegate` share `admit_current`. `cargo clippy --all-targets -p loopflow-execution -p loopflow -- -D warnings` clean; `cargo nextest run -p loopflow-execution --lib -E 'test(transport::launch_mode_tests::) | test(human_session::)'` passed 51/51.

## Review answers, 2026-11-17, second review (`scratch/concept-review.html`)

Jack: "1a 2 --d ont really care but dont be biased by my previous answers. just
do whatevers cleanest 3b. thats landing soon tho so tell them asap 4a 5n".
"5n" is read as 5b (hand to LOO-958); n sits beside b.

Built 2026-11-17, no behavior change: `lf task run` detaches through
`transport::detach_current` before any launch, so `delegate` hands only a work
session to the holder; `harness::Run` is `LaunchKind` (fields and variables
`kind`, `transport::session_kind`); the glossary rows lead with what a person
sees and say a Session can change kind.

Not in this PR, handed to LOO-958: the refusal text "`ID is a Run (…); lf
resume takes only Sessions`", and checking whether a work session that is
neither bound nor primary reopens from a terminal as a chat. Both were
steered into LOO-958, along with the `LaunchKind` rename underneath it.
