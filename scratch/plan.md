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

### Executable UX parity contract (`harness::parity`)

Create `rust/loopflow-execution/src/harness/parity.rs`:

- `pub enum Behavior` (exhaustive, with `Behavior::ALL`)
- `pub enum Support { Yes, Gap { why: &'static str, owner: &'static str } }`
- `pub enum Verification { Automated(&'static str), RealProvider(&'static str) }`
- `pub fn support(harness: HarnessKind, behavior: Behavior) -> Support` — exhaustive
  match over `(HarnessKind, Behavior)` with no wildcard arms.
- `pub fn verification(behavior: Behavior) -> Verification` — distinguishes the 12
  behaviors verified across `HarnessKind::ALL` by automated tests from behaviors
  whose real-provider failure cannot be reproduced honestly by a stand-in script
  (currently `ViewerHandoff` / LOO-1097).

| Behavior | Verification | Current gaps |
|---|---|---|
| `WorkerStep` (runs, settles, records answer & thread, Graph advances) | Automated (`a_task_step_runs_as_a_worker_on_every_harness`) | none |
| `UsageRecording` (records usage in Task history) | Automated (same test) | none |
| `StepContext` (receives step context) | Automated (same test) | none |
| `MidTurnSteer` (hears steer after next tool) | Automated (same test) | none |
| `HeadlessPermissions` (never asks permission whatever `yolo` says) | Automated (`harness::launch_matrix`) | none |
| `MissingCli` (passed over when uninstalled, or named once when sole choice) | Automated (`a_step_that_cannot_launch_is_not_started_again`) | none |
| `UnansweredQuestion` (fails turn with question when nobody is there) | Automated (`a_question_with_nobody_there_ends_the_step`) | Claude, Codex, Antigravity (Task 48edb9) |
| `Interrupt` (stops on `lf task interrupt`, next run retries step) | Automated (`a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again`) | none |
| `RefusalFallthrough` (classifies quota, revoked login, unserved model) | Automated (`conformance_tests::each_harness_classifies_its_own_refusals`) | none |
| `IdleWorkSessionWake` (idle work session hears a steer) | Automated (`server::wake::tests::every_harness_wakes_its_idle_work_session`) | Antigravity (LOO-1098) |
| `ReopenWaitingSteers` (reopened work session takes waiting steers) | Automated (`server::wake::tests::every_harness_takes_waiting_steers_on_reopen`) | none |
| `InstalledSkills` (synced skills & Graphs listed) | Automated (`conformance_tests::every_harness_reads_a_synced_skill`) | none |
| `ViewerHandoff` (opening in another viewer keeps the conversation) | RealProvider (`transport_tests` proves held-terminal sharing; provider TUI checked live) | Antigravity (LOO-1097) |

## Conformance

Every `Verification::Automated` behavior test loops over `HarnessKind::ALL` with
an exhaustive `match` (never a hardcoded subset or string array) and checks
`parity::support(harness, behavior)` both ways: `Support::Yes` must pass, and
`Support::Gap` must still exhibit the gap, so fixing a gap fails the test until
`parity.rs` and the README are updated. Failure messages name `{harness:?}` and
the behavior.

Specifically:
- `server::wake::tests::every_harness_wakes_its_idle_work_session`: loops
  `HarnessKind::ALL`, sets up each harness's idle reach from
  the supported provider routes, saves a steer and checks delivery against
  the support cell, including an unread steer for a gap.
- A reopen test over `HarnessKind::ALL` should exercise actual `lf resume`
  with a saved thread and waiting news. Observe the provider starting its
  turn before anyone types and the same Session's delivery receipt.
- `task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness`: switch
  the hardcoded 5-tuple array to `for harness in HarnessKind::ALL`.
- `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again`: check
  missing-CLI install hint and pass-over across `HarnessKind::ALL`.
- `task_graph_launch_tests::a_question_with_nobody_there_ends_the_step`: loop
  `HarnessKind::ALL`, asserting `Support::Yes` fails with `unanswered_question`
  and `Support::Gap` does not.
- Unify the three split interrupt tests (`…_on_claude_and_codex`,
  `…_on_opencode_and_pi`, `…_on_antigravity`) into one loop over `HarnessKind::ALL`.
- `harness::conformance_tests::each_harness_classifies_its_own_refusals`: loop
  `HarnessKind::ALL` with an exhaustive `match` so every harness has refusal
  classification fixtures.
- `parity::tests::readme_table_matches_support`: renders the parity table between
  markers (`<!-- parity:start -->` / `<!-- parity:end -->`) in
  `rust/loopflow-execution/src/harness/README.md` and asserts exact match.
  `cargo test -p loopflow-execution --lib harness::parity::tests::regenerate_readme_table -- --ignored`
  rewrites it. Update `rust/loopflow/src/server/README.md` ("Waking an idle
  conversation") to describe the implemented delivery mechanism.

## Exclusions

- Real-provider viewer handoff for `agy` (LOO-1097): tracked as a typed
  `RealProvider` `Gap` owned by LOO-1097, not fixed here.
- Reading a headless question on Claude, Codex, or Antigravity: tracked as a
  typed `Automated` `Gap` owned by Task 48edb9.
- Offline gate check verifying that a `Gap`'s owner Task is still open.

## Remaining work

- Find a safe way to wake an *already open* idle Antigravity conversation.
  Keep the last step's receiver, single delivery, focus, menus and draft
  requirements. Server replacement must retain enough information to avoid
  submitting a person's draft.
- Build the typed parity table and tests over every harness as specified
  above. Keep the Antigravity idle-wake cell a gap owned by LOO-1098 until
  the live demo works. Reopen acceptance should exercise the public resume
  entry point, provider turn and stored delivery receipt.
- Regenerate the table and bring the provider checklist and server docs into
  agreement with the implemented contract.

## Acceptance at gate

```sh
cargo test -p loopflow-terminal --lib ghostty_vt::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::parity::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib transport::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
```

Live demo still required for delivery to an already open idle conversation.

## Slice checks

- `session_launch_contract::a_reopened_antigravity_task_hears_repository_direction_once` passed through `lf resume --task` for repository direction, a Task update and an empty inbox. The first fixture run selected the installed agy from the server's PATH. The fixture now pins its stand-in on the client and server.
- `session_launch_contract::task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings` passed with the same fixture PATH.
- `harness::environment_tests::a_nested_provider_cannot_address_the_parent_antigravity_conversation` passed.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Gate and CI own the remaining suites.
