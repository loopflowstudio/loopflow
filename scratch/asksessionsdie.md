# Remove Ask

Accepted direction — Jack Heart, 2026-10-01. Implementation complete locally;
realigned 2026-10-01 after compression and upstream integration. Full Rust and
Desktop acceptance remains with gate. Checkpoint publication was requested after
realignment; landing remains outside this operation.

## Intended outcome and accepted boundary

Headless work that cannot proceed explains the impediment in its normal output
and ends through the existing failure contract. The responsible Wave operator
reads status and logs, resolves authorized impediments, and discusses necessary
judgment in its existing Wave chat. Taskless failure returns to its caller.
There is no escalation command, replacement queue, automatic child chat,
notification, polling waiter, or mandatory agent-authored handoff.

Jack’s direction: “mostly headless agents should just fail and say they failed
if they dont have what they need”. He clarified “no, Task sessions stay”. Task
conversations and explicitly authored `human: true` reviews retain their feedback
and completion contract. Wave chat does not replace or complete those reviews.
Persistent versus one-off Task conversations remains open and outside this change.

A normal successful provider turn is not reclassified from prose that mentions
difficulty. Existing provider errors and structured blocked outcomes establish
failure; ordinary output remains evidence. Missing logs or failed status reads
remain unknown. Reading evidence grants no process or delivery authority.

## Delete — do not maintain

Removed: `lf/commands/ask.rs`, the builtin `task/skill/unblock.md`, Ask kinds
and tokens, `reserve_ask`, `ask_once`, `task_unblock`, `flow_unblock`,
`answer_flow_blocker`, and their exclusive tests/fixtures. No remaining Ask
runtime deletion targets were found. Retain historical migrations and exchange
tables, and the shared conversation launch and Task review paths.

## Implemented behavior and findings

The follow-up compression removes `open_waiting`'s pre-lock read and retry
loop: one read under the launch lock already selects the latest input and
rejects concurrent completion. Its unused artifact-key return is removed too.
The review keeps native resume, input publication and completion checks intact;
a focused race test proves that completion while opening waits cannot relaunch
the conversation or append an input.

- Driver and controllers no longer open Ask or consume its feedback. Blocked
  verdicts reach `fail_flow`, retain their cursor and logs, and return failure.
  Removing the driver interception alone was insufficient: `project_output` in
  `store/sqlite/flows.rs` also hid blocked verdicts and caused repeated checkpoints.
  That exception is removed, with saved-verdict recovery coverage.
- Scheduled Task reconciliation leaves recorded failures stopped. Existing bounded
  launch recovery remains. Explicit `--retry` retains the same position and
  conversation; Task recovery passes its trimmed `--reason` into `retry_flow`.
- Ask CLI, runtime/wire kinds, reservation, wait/feedback helpers, automatic
  callers and the Ask-only unblock skill are removed. Rust/Swift DTO fixtures,
  builtin prompts and goldens reflect the surviving conversation/review model.
- Shared primary launch uses `serve-conversation`, `conversation_exec_is_running`,
  `conversation_background_name` and `conversation_launch_args`. Existing primary
  terminal identity, prepared input, locks, native resume and provider generations
  remain. Ordinary Home launch replaces Ask-only environment injection.
- The `remove_ask` migration draft depends on `primary_session_scope`. It converts
  former Ask rows to conversations, clears primary scope and the retired unblock
  skill, and preserves identity, request, native history, attribution and completion.
  Released migrations, historical exchange tables and immutable events remain.
  Conversion launches nothing and completes nothing. Explicit opening of an
  unlaunched converted conversation retains its request context.
- Completed historical Asks confer no Flow settlement authority. Saved blocked
  verdicts reach ordinary failure on recovery; already recorded failures stay
  stopped. Cross-version live driver handoff is not promised.

Paths above are relative to `rust/loopflow/src/`.

## Reconciliation findings

The merged upstream context-cost instrument reads retained Session events and adds
weekly usage dispatch; inspection found no new Ask dependency or change to failure
settlement. Its historical evidence limits remain in Intelligence memory.

Source review found stale active-Ask descriptions in the architecture reference,
admission docs, getting-started guide and Swift README. These now describe ordinary
conversations and retained reviews. Product’s existing objective and memory also
carried Ask caller-release requirements; their Session/review guidance now records
Jack’s dated boundary while preserving historical observations and outstanding
review proof. Release notes and historical design reviews remain historical.

No exact Wave placement was supplied. Product memory is updated as the existing
owner of the shared Session contract; this does not assign the work to that Wave.

## Remaining acceptance

Gate owns the full materialized Rust suite and complete Desktop suite. Earlier
focused evidence covers failure/retry, reconciliation, conversation launch/reopen,
migration preservation, DTOs and prompt goldens. Acceptance is deliberately split:

- Public CLI provider simulations cover a Task-associated provider failure and
  a taskless blocked decision, nonzero return, retained history, explicit retry
  in the same conversation and position, and subsequent Task review completion.
  The taskless blocked case has a deadline and proves repeated resume does not
  launch or advance. The Task-associated Flow is not the Task’s managed Flow.
- Public Task reconciliation starts from a recorded managed failure and proves
  two reconciliations preserve failed status and reason without launching sessions
  or retrying. Explicit store retry preserves position and carries new direction.
- Store reopen at a saved blocked verdict proves recovery reaches failure without
  moving the cursor; explicit retry retains the pass and conversation.
- Migration fixtures cover completed, open-native and reserved-unlaunched Asks.
  Primary launch/reopen, CLI rejection and retained review tests cover the shared
  paths. Desktop DTO/store coverage must preserve Task review actions.

Together these must prove no automatic Ask creation, no silent retry or premature
advancement, retained historical conversations, and surviving Task review feedback.
No live provider or display is required. Broader configured-provider and native UI
judgment are not established by these simulations.

Remaining gate commands:

```sh
uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build -p loopflow --bin lf
scripts/test_desktop.sh
```

Compression recorded passes for `cargo test -p loopflow --lib ops::human_session`
(21 tests), the Task-associated `failure_without_ask` lifecycle case,
`cargo build -p loopflow --bin lf`, formatting and clippy before upstream integration.
Gate determines reuse against its exact content and command plan; integration does
not itself require this reconciliation to repeat those checks.

Check: 2026-10-01 — realign: `uv run python scripts/check_architecture.py` and `git diff --check` passed for documentation repairs; full materialized Rust/Desktop acceptance deferred to gate.

Publication review: the diff against `origin/main` preserves the accepted failure,
conversation migration and Task review boundaries. Fixed TESTING.md's stale
AskHome fixture reference. This is a checkpoint PR with scratch retained; prior
focused checks are recorded above, and full post-integration acceptance remains
with gate. No live-provider or native UI proof is claimed.

Check: 2026-10-01 — follow-up compress: `cargo test -p loopflow --lib ops::human_session` passed (22 tests); `cargo build -p loopflow --bin lf`, `cargo fmt --all -- --check` and `git diff --check` passed; full Rust/Desktop acceptance remains with gate.
