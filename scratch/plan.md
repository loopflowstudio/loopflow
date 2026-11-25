# LOO-1098: wake idle Antigravity, and make harness parity typed

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html).
In one line: idle wake was inferred in SQL from whichever columns a harness filled
in, parity gaps were hand-written README cells, and feature tests hardcoded subsets
of harnesses. So `agy` fell through without any error.

## Implementation finding, 2026-11-24

A disposable PTY running agy 1.5.16 emitted `ESC[?2004h` at the workspace
trust dialog. With a trusted workspace, the slash menu, the model picker
opened with Enter, and the return to the prompt produced no paste-mode
transition. A server can also adopt a terminal whose TUI still holds a draft
while its new Hosted has no keystroke history. Paste mode cannot establish a
safe destination for a steer. **Stop the held-terminal wake design.** Keep
steers pending on an already open, idle agy until there is a provider-supported
way to address its conversation, or a reliable indication that its empty prompt
alone will consume the input. Any replacement has to pass these live checks
and preserve drafts across server replacement.

A separate disposable conversation started in print mode and reopened with
`agy --conversation <id> -i <prompt>` recorded its second USER_INPUT on the
original transcript. The PreInvocation hook ran before anyone typed, and Stop reported the end of the turn.
Reopen needs neither a synthetic launch boundary nor terminal readiness:
materialize Task updates, ask whether the work session has unread steers, and
use an opening prompt to start the hook that delivers them. An empty queue
reopens quietly. Keep the current path for bare launches and saved inputs
without hooks. Named steers can still start stopped conversations; a broader
steer on its own only starts a turn when someone reopens the conversation.

Private Machine, real agy 1.5.16: quit the Task conversation, save a repository
steer, then run `lf resume --task <id>`. The original transcript recorded a
second user turn and the exact answer `VIOLET`. `lf session steers` showed
one receipt for that steer and zero pending. Both runs exited.

## Source of truth

### Runtime idle wake

Choose Antigravity's live route before changing SQL reach resolution.
The existing inbox, app-server and socket continue to serve the other four.
Keep the per-harness declaration and shared reach resolver as requirements
when that route is known. The earlier probe notes in
`93d232bf1d:scratch/plan.md` record which hooks and sidecars receive an
Antigravity server address on 1.5.16.

### Executable UX parity contract

`harness::parity::support` declares 13 behaviors for every `HarnessKind`.
The harness README is generated from those cells. Twelve rows name automated
checks. Viewer handoff needs a live provider and its Antigravity gap belongs
to LOO-1097. Headless questions on Claude, Codex and Antigravity belong to
Task 48edb9.

Task Graph fixtures start workers, steer them during a tool call, interrupt
and retry them, ask a question, and exercise both missing-CLI choices. The
wake fixture checks the provider's addressed message and the Session's
receipt. It covers the first boundary after reopen on Claude, Codex,
OpenCode and Pi. Antigravity starts its turn earlier, at launch, so its cell
also names the existing `lf resume --task` fixture.

## Remaining work

Already-open Antigravity idle wake belongs to LOO-1099. Revisit a provider
route there with the draft, dialog and server-replacement cases above.
Gate owns the acceptance list and the live reopen demo. Gap owners are
recorded in the table. Checking their tracker state offline is out of scope.

## Acceptance at gate

```sh
cargo test -p loopflow-terminal --lib ghostty_vt::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::parity::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib harness::launch_matrix::every_harness_runs_all_four_launch_kinds
cargo test -p loopflow-execution --lib transport::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

LOO-1099 owns the live demo for an already-open idle Antigravity conversation.

## Slice checks

- `session_launch_contract::a_reopened_antigravity_task_hears_repository_direction_once` passed through `lf resume --task` for repository direction, a Task update and an empty inbox. The first fixture run selected the installed agy from the server's PATH. The fixture now pins its stand-in on the client and server.
- `session_launch_contract::task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings` passed with the same fixture PATH.
- `harness::environment_tests::a_nested_provider_cannot_address_the_parent_antigravity_conversation` passed.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Gate and CI own the remaining suites.
- The first missing-CLI run found Homebrew OpenCode after the fixture removed it from PATH. The worker ran the installed CLI and reported `ECONNREFUSED`. The fixture now hides system provider executables for its process tree.
- The parity table regenerated and its drift check passed. The 10 named behavior fixtures passed on macOS, including the missing-CLI fixture with three installed provider executables hidden. Linux CI owns the private mount check. The macOS debug linker warned about the size of the unwind table.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and Ruff passed for the parity change.
