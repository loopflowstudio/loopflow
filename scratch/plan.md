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

## Pass 5 — Concept review fixes (harness-owned terminal readiness & immediate deferred-wake nudge)

Post-build concept review against base `989b41c3bb` (`scratch/concept-review.html`) found two conceptual mismatches introduced in Pass 4 that should be fixed before gate:

### 1. Move terminal prompt-readiness ownership out of generic `transport/` onto `IdleReach::Terminal`

- **Problem:** `Hosted::wake_idle_prompt` (`rust/loopflow-execution/src/transport/hosted.rs:245`) hardcodes `crate::harness::antigravity::prompt_ready` — the only provider-specific import in all of `transport/`. Any future harness returning `IdleReach::Terminal` would compile cleanly and silently test its screen against Antigravity's box/footer layout.
- **Model to build:**
  - `loopflow_execution::steer::IdleReach::Terminal` carries both the terminal and the harness's readiness function:
    ```rust
    pub enum IdleReach {
        Codex(std::path::PathBuf),
        Inbox(Inbox),
        Terminal {
            terminal: loopflow_model::id::TerminalId,
            prompt_ready: fn(&loopflow_terminal::ghostty_vt::Terminal) -> bool,
        },
    }
    ```
    (Drop unused `PartialEq, Eq` derives on `IdleReach` so clippy never sees function-pointer comparisons; keep `Debug, Clone`.)
  - `Antigravity::idle_reach` returns `endpoints.held_terminal.clone().map(|terminal| IdleReach::Terminal { terminal, prompt_ready })`, forcing any future terminal-woken harness to supply its own predicate.
  - `Registry::wake_idle_prompt(&self, terminal: &TerminalId, prompt: &str, prompt_ready: fn(&Terminal) -> bool)` -> `Command::WakePrompt { terminal, prompt, prompt_ready, reply }` -> `Hosted::wake_idle_prompt(&mut self, child, prompt, prompt_ready)` checks `self.output.terminal().is_some_and(prompt_ready)`. Remove the `crate::harness::antigravity` import from `transport/hosted.rs`.
  - In `server/wake.rs::wake`, match `IdleReach::Terminal { terminal, prompt_ready }` first before unpacking `let Some(thread) = thread.as_deref() else { return Ok(false); }` for the provider-API variants (eliminating the unreachable `(_, None)` match arm).
  - Update `transport/registry_tests.rs::wake_idle_prompt_guards_against_drafts_typing_and_open_pastes_but_ignores_mouse_and_focus` to pass `crate::harness::antigravity::prompt_ready` (and verify a custom predicate works too).

### 2. Wake `Watcher` immediately on deferred turn-end wakes (remove process-global `static DEFERRED`)

- **Problem:** When a steer arrives during a turn on an open Antigravity session and the person has a draft/menu open (or `agy` redraws >600 ms after `Stop`), `boundary::answer`'s `session_activity` write does not bump `task_update_revision` or `steer_revision`, so `Watcher` goes right back into `StoreChanges::wait(60s)` (blocking inside `kqueue` on macOS). `boundary::wake` then sets `static DEFERRED: AtomicBool = true` without waking `kqueue` — causing up to a **60-second stall** after the person clears their draft, and leaking global state across tests.
- **Model to build:**
  - Add a thread-safe `StoreWaker` handle to `loopflow_storage::changes::StoreChanges` (`StoreChanges::waker(&self) -> StoreWaker`):
    - **macOS:** Create the `kqueue` once in `StoreChanges` with an `EVFILT_USER` event registered, and attach/re-attach `EVFILT_VNODE` on the WAL fd inside that same `kqueue` as the WAL appears or rotates (if the WAL does not exist yet, still wait on the `kqueue` up to `timeout.min(100ms)` so `waker.wake()` breaks out immediately). `StoreWaker::wake(&self)` posts `EVFILT_USER | NOTE_TRIGGER` to the `kqueue` fd (keep the `kqueue` fd behind `Arc<OwnedFd>` so closing `StoreChanges` safely outlives or drops cleanly).
    - **non-macOS:** Wait on a `Mutex<bool>` + `Condvar` for `timeout.min(50ms)`; `StoreWaker::wake(&self)` sets the flag and calls `notify_one()`.
  - Add a unit test in `loopflow-storage/src/changes.rs` proving `waker.wake()` ends a 20 s `changes.wait` immediately (even with no WAL write).
  - Replace `static DEFERRED: AtomicBool` and `wake::note_deferred()` in `rust/loopflow/src/server/wake.rs` with an instance-owned `WakeNudge` (`Arc<(AtomicBool, StoreWaker)>`) returned by `wake::start(terminals)` and passed through `service.rs` to `boundary::after` / `boundary::wake`:
    - `nudge.retry_deferred()` sets its `AtomicBool` and calls `waker.wake()`.
    - `watch` checks `watcher.deferred || nudge.is_pending()` to select `DEFERRED_RETRY` (2 s), and `Watcher::read` swaps the instance flag to trigger `wake_idle` immediately.
  - Add a focused test in `server::wake::tests` proving `WakeNudge::retry_deferred` wakes a waiting watcher loop out of `UNSIGNALLED` (60 s) without any store revision move.

### Execution order

1. Start with `loopflow-storage/src/changes.rs` (`StoreWaker` + unit test) — smallest, most fundamental piece that proves immediate wakeup on macOS `kqueue`.
2. Wire `WakeNudge` through `server/{wake,boundary,service}.rs`, removing `static DEFERRED: AtomicBool`, and add the watcher wakeup test.
3. Refactor `IdleReach::Terminal` + `Registry`/`Hosted::wake_idle_prompt` across `steer.rs`, `harness/antigravity.rs`, `transport/{hosted,registry,registry_tests}.rs`, and `server/wake.rs`.
4. Run focused tests + `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`.

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
