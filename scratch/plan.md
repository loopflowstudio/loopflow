# LOO-1098: wake idle Antigravity work sessions and enforce harness parity

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html). Live QA: [qa.html](qa.html).

## Built & verified at `771d2d2967` (Passes 1–4)

- **Antigravity reopen with waiting steers & Task updates:** `agy --conversation <id> -i "Read the waiting steers and Task updates for this Session."` fires `PreInvocation` before anyone types, claims and stamps steers only inside the hook, and omits `-i` when nothing is waiting. Verified live on real `agy 1.5.16` and `1.5.17`.
- **Required harness interface & unconditional parity tests:** Removed `harness/parity.rs` and the generated README parity table per Jack Heart's direction. `Harness` requires `recognizes_driver`, `caller_session_env`, `sign_in_advice`, `transcript_home`, and `transcript_reader`; `Launcher` requires `capabilities`, `idle_reach`, `classify_failure`, `open`, and `driver`; `Capabilities` has no `Default` and separates `initial_prompt_on_resume: bool` (Codex empty-thread replay) from `reopen_steers: ReopenSteers::{ServerBoundary, PromptHook}`. All behavior tests iterate `HarnessKind::ALL` with only one narrow inline `TODO` arm left, for pre-existing `Task 48edb9` (headless unanswered questions).
- **Harness-owned idle-wake endpoints:** `sqlite/steers.rs::idle_conversation_candidates` returns raw `IdleCandidate` rows (`INDEXED BY agent_sessions_unended_agent`, allowing `provider_thread IS NOT NULL OR held_terminal_id IS NOT NULL`); `loopflow-execution/src/steer.rs` routes both server wake (`idle_conversations`, skipping OS process checks when `idle_reach` is `None`) and hook start checks (`is_idle_reachable`) through `Launcher::idle_reach`. Live QA on real Claude, Codex, and Antigravity confirmed this also closes a store hazard where 974 Codex rows had `agent_inbox` populated.
- **Live already-open idle Antigravity wake via VT-guarded held terminal:**
  - Global sidecar route was probed and ruled out on `agy` CLI 1.5.17 (App-only).
  - `loopflow-terminal` (`ghostty_vt.rs`) exposes `Terminal::bracketed_paste()`, `Terminal::text()`, and `Terminal::cursor()` in production builds.
  - `Antigravity::open` records `record_first_boundary` best-effort when opened managed without `-i`; `lf/commands/context.rs::read_news` checks `!prompt` so `PreInvocation` always claims steers; `WAKE_PROMPT` is shared across reopen and idle wake.
  - `antigravity::prompt_ready` guards alternate screen, bracketed paste, ground parser state, OSC 7501 `Working`/`Blocked`, empty prompt box (bare `>` or `agy 1.5.17`'s dim `(shift+tab to cycle)` mode hint with cursor parked right after `> `), `? for shortcuts` footer, and absence of cancel/navigation overlays.
  - `transport::input::has_user_keystroke` ignores mouse, focus, and query replies while holding off wakes for 2 s after real user keystrokes, open pastes, pending input, or post-restart ring replay.
  - All 5 harnesses in `HarnessKind::ALL` pass `server::wake::tests::every_harness_wakes_its_idle_work_session` unconditionally.
  - Live QA of Pass 4 passed on real `agy 1.5.17` across 8 scenarios (first idle prompt with mode hint, server restart recovery, open idle wake, draft hold + release, slash menu hold + release, `/model` draft hold + release, settled Graph run handoff, unviewed session wake).
- **Inline steer display in Antigravity:** Native QA on `agy 1.5.16`/`1.5.17` proved `metadata.user_markdown` causes `type mismatch: cannot convert string to message` and drops the hook reply; `renderInfo.markdown` stays hidden; `userMessage` writes `USER_INPUT` from `SYSTEM_SDK`. Prototype withdrawn, fixture updated to model the drop, and inline display split to **LOO-1103** (`task_a265825e5d3e484b88cd7c047a79b675`) per Jack Heart's `(b)` choice.
- **Missing-CLI sandbox alignment:** `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again` checks the same four system tool dirs (`/opt/homebrew/{bin,sbin}`, `/usr/local/{bin,sbin}`) hidden by `scripts/test_network.py`.

## Pass 5 — Concept review fixes (built & verified at `e7f7746196`)

Post-build concept review against base `989b41c3bb` (`scratch/concept-review.html`) found two conceptual mismatches introduced in Pass 4; both are now resolved:

1. **Harness-owned terminal prompt-readiness (`IdleReach::Terminal`):**
   - `loopflow_execution::steer::PromptReady` (`fn(&Terminal) -> bool`) is carried on `IdleReach::Terminal { terminal, prompt_ready }` (dropping unused `PartialEq, Eq` derives on `IdleReach`).
   - `Antigravity::idle_reach` supplies `antigravity::prompt_ready`, so any future terminal-woken harness must supply its own screen-readiness guard.
   - `Registry::wake_idle_prompt` and `Hosted::wake_idle_prompt` accept `prompt_ready`, removing the last provider-specific import from `transport/`.
   - `server/wake.rs::wake` matches `IdleReach::Terminal { terminal, prompt_ready }` first before unpacking `thread` for provider-API variants.
   - `transport::registry::tests::wake_idle_prompt_guards_against_drafts_typing_and_open_pastes_but_ignores_mouse_and_focus` passes `antigravity::prompt_ready` and verifies a custom rejecting predicate blocks injection.

2. **Immediate deferred-wake nudge (`StoreWaker` + instance-owned `WakeNudge`):**
   - `loopflow_storage::changes::StoreChanges` creates its `kqueue` once up front with `EVFILT_USER` on macOS (attaching/detaching `EVFILT_VNODE` on the WAL fd inside the same `kqueue`) and uses `Mutex<bool>` + `Condvar` elsewhere; `StoreChanges::waker(&self) -> StoreWaker` wakes `StoreChanges::wait` immediately without touching SQLite.
   - `changes::tests::waker_ends_the_wait_without_a_wal_write` proves immediate wakeup before a WAL exists, while armed on a WAL with no writes, and clean reset afterward.
   - Removed process-global `static DEFERRED: AtomicBool` and `wake::note_deferred()`; `wake::start` returns `Option<WakeNudge>` (`Arc<(AtomicBool, StoreWaker)>`), threaded through `service.rs` to `boundary::after` / `boundary::wake`.
   - `server::wake::tests::wake_nudge_wakes_waiting_watcher_without_a_revision_move` proves `WakeNudge::retry_deferred()` breaks a watcher out of `UNSIGNALLED` (60 s) and wakes an idle Antigravity terminal with no revision move or WAL write.

Verified: `changes::tests` (4/4), `transport::registry::tests::wake_idle_prompt_*` (1/1), `server::wake::tests` (15/15), `server::boundary::tests` (14/14), `context_launch_tests::` recovery fixtures (`a_task_checkout_starts`, `a_killed_task_conversation`, `a_task_conversation_whose_agent`, 3/3), `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`. `origin/main` (`d8b2afebd9`) merged afterward at `5004136804`; its only overlap with this work is `harness/codex.rs`, not rerun locally. Nothing remains to build; next is gate.

## Acceptance at gate

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -p loopflow-storage --lib changes::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-terminal --lib ghostty_vt::tests::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib harness::launch_matrix::every_harness_runs_all_four_launch_kinds
cargo test -p loopflow-execution --lib harness::environment_tests::
cargo test -p loopflow-execution --lib harness::antigravity::
cargo test -p loopflow-execution --lib transport::input::
cargo test -p loopflow-execution --lib transport::registry::tests::wake_idle_prompt_
cargo test -p loopflow-execution --lib steer::
cargo test -p loopflow-execution --lib context_block::
cargo test -p loopflow --lib lf::commands::context::tests::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
cargo test -p loopflow --test integration documented_commands::
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

## Outside this work

- Inline steer & Task-update display in Antigravity TUI: **LOO-1103** (`task_a265825e5d3e484b88cd7c047a79b675`, waiting on a working display-only `agy` hook field).
- Viewer handoff (multi-viewer `agy`): **LOO-1097**.
- Headless unanswered questions on Claude, Codex, Antigravity: **Task 48edb9**.
- Claude cross-session steer framing (`scratch/qa.html` pass 2 #1: Claude `task-session` primaries acknowledge `<cross-session-message>` steers and wait for typed confirmation while Codex and Antigravity act immediately): separate prompt/framing outcome outside this diff.
