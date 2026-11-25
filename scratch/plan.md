# LOO-1098: wake Antigravity on reopen, display steers inline in `agy`, and enforce harness parity

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html). Live QA: [qa.html](qa.html).

## Built & verified at `76ff5e7438` (passes 1–2)

- **Antigravity reopen with waiting steers & Task updates:** `agy --conversation <id> -i "Read the waiting steers and Task updates for this Session."` fires `PreInvocation` before anyone types, claims and stamps steers only inside the hook, and omits `-i` when nothing is waiting. Verified live on real `agy 1.5.16` (pass 1 at `c0b28b7dc6`, pass 2 at `4414e34eb5`).
- **Required harness interface & unconditional parity tests:** Removed `harness/parity.rs` and the generated README parity table per Jack Heart's direction. `Harness` requires `recognizes_driver`, `caller_session_env`, `sign_in_advice`, `transcript_home`, and `transcript_reader`; `Launcher` requires `capabilities`, `idle_reach`, `classify_failure`, `open`, and `driver`; `Capabilities` has no `Default` and separates `initial_prompt_on_resume: bool` (Codex empty-thread replay) from `reopen_steers: ReopenSteers::{ServerBoundary, PromptHook}`. All behavior tests iterate `HarnessKind::ALL` with narrow inline `TODO` arms only for pre-existing `LOO-1099` and `Task 48edb9`.
- **Harness-owned idle-wake endpoints:** `sqlite/steers.rs::idle_conversation_candidates` returns raw `IdleCandidate` rows (`INDEXED BY agent_sessions_unended_agent`, 3.63 ms for 200 unfinished rows / 0.035 ms for one session); `loopflow-execution/src/steer.rs` routes both server wake (`idle_conversations`, skipping OS process checks when `idle_reach` is `None`) and hook start checks (`is_idle_reachable`) through `Launcher::idle_reach`. Live QA on real Claude, Codex, and Antigravity passed and confirmed this closes a real store hazard (974 Codex rows in `~/.lf/loopflow.db` had `agent_inbox` set; Codex now reads only `agent_endpoint`).
- **Missing-CLI sandbox alignment:** `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again` checks the same four system tool dirs (`/opt/homebrew/{bin,sbin}`, `/usr/local/{bin,sbin}`) hidden by `scripts/test_network.py`.

## Pass 3 design — display incoming steers & Task updates inline in Antigravity

Direction from Jack Heart (2026-11-25, `comment:79dd230d-1028-4f14-acb2-90b07b1f3804`):
- `"is this also ging to make it so that agy *displays* the steers it gets"`
- `"Just go ahead and put this in the task please, lets go back"`

### Result of the native terminal check, 2026-11-25

The proposed hook format loses steers. In a disposable `agy` 1.5.16
conversation reopened with `--conversation … -i`, the terminal completed the
turn and the hook log read `type mismatch: cannot convert string to message`.
The same thing happened on 1.5.17 on startup and on a later typed turn. One
`systemMessage` with a string at `metadata.user_markdown` discarded every
step in that hook reply, including the ordinary reference step. lf records
delivery after printing a hook reply, before the provider parses it.

Two other shapes ran in the same terminal:

- `systemMessage.renderInfo.markdown` let the model receive the system
  text. Both versions answered `MODELWORD`. The separate display marker
  stayed off screen, including with a title and `hidden: false`.
- A `userMessage` on 1.5.17 showed the marker above the response. The native
  transcript stored it as `USER_INPUT` from `SYSTEM_SDK`, inside
  `<USER_REQUEST>`. The lf transcript reader starts a new turn at that row
  and attributes unwrapped text to the person. Task updates would reach
  the model as requests.

The display prototype was withdrawn. The provider fixture now drops the same
invalid hook reply as the real CLI, so the existing Task reopen contract
catches this loss. The Pass 2 reopen path still sends plain system messages.

### Resolved decision (2026-11-25)

Jack Heart chose `(b)`: ship Pass 2 with today's system-message delivery and track inline steer/Task-update display in its own Task (**LOO-1103**, `task_a265825e5d3e484b88cd7c047a79b675`). No further implementation remains in LOO-1098; proceed to `pr-publish` and `ship-review`.

### Focused check

`cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once` passed with the corrected fixture (1 passed, 28 filtered). Native results are in [QA](qa.html).

## Acceptance at gate

```sh
cargo test -p loopflow --test integration documented_commands::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib harness::launch_matrix::every_harness_runs_all_four_launch_kinds
cargo test -p loopflow-execution --lib harness::environment_tests::
cargo test -p loopflow-execution --lib harness::antigravity::
cargo test -p loopflow-execution --lib steer::
cargo test -p loopflow-execution --lib context_block::
cargo test -p loopflow --lib lf::commands::context::tests::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

## Outside this work

- Inline steer & Task-update display in Antigravity TUI: **LOO-1103** (`task_a265825e5d3e484b88cd7c047a79b675`, waiting on a working display-only `agy` hook field).
- Already-open Antigravity idle wake: **LOO-1099** (live PTY probes ruled out `?2004h` paste-mode readiness).
- Viewer handoff (multi-viewer `agy`): **LOO-1097**.
- Headless unanswered questions on Claude, Codex, Antigravity: **Task 48edb9**.
- Claude cross-session steer framing (`scratch/qa.html` pass 2 #1: Claude `task-session` primaries acknowledge `<cross-session-message>` steers and wait for typed confirmation while Codex and Antigravity act immediately): separate prompt/framing outcome outside this diff.
