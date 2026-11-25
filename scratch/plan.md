# LOO-1098: wake Antigravity on reopen, display steers inline in `agy`, and enforce harness parity

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html). Live QA: [qa.html](qa.html).

## Built & verified at `76ff5e7438` (passes 1–2)

- **Antigravity reopen with waiting steers & Task updates:** `agy --conversation <id> -i "Read the waiting steers and Task updates for this Session."` fires `PreInvocation` before anyone types, claims and stamps steers only inside the hook, and omits `-i` when nothing is waiting. Verified live on real `agy 1.5.16` (pass 1 at `c0b28b7dc6`, pass 2 at `4414e34eb5`).
- **Required harness interface & unconditional parity tests:** Removed `harness/parity.rs` and the generated README parity table per Jack Heart's direction. `Harness` requires `recognizes_driver`, `caller_session_env`, `sign_in_advice`, `transcript_home`, and `transcript_reader`; `Launcher` requires `capabilities`, `idle_reach`, `classify_failure`, `open`, and `driver`; `Capabilities` has no `Default` and separates `initial_prompt_on_resume: bool` (Codex empty-thread replay) from `reopen_steers: ReopenSteers::{ServerBoundary, PromptHook}`. All behavior tests iterate `HarnessKind::ALL` with one narrow inline `TODO` arm left, for pre-existing `Task 48edb9` (headless questions); the `LOO-1099` arm went in Pass 4.
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

Jack Heart chose `(b)` for inline display: ship with today's system-message delivery and track inline steer/Task-update display in its own Task (**LOO-1103**, `task_a265825e5d3e484b88cd7c047a79b675`).

## Pass 4 — Live already-open idle Antigravity wake via VT-guarded held terminal

At `ship-review` (2026-11-25), Jack Heart directed folding already-open idle Antigravity wake back into LOO-1098 (with Option A: trigger prompt via `PreInvocation`) and abandoning LOO-1099:
- `"wait this was deferred? isnt this the main thing i asked for? Already-open idle agy wakeLive idle agy work sessions do not wake until a typed turn or reopen; blocked on a safe provider route."`
- `"Test the sidecar route now ( and then Revisit held-terminal wake if necessary"`
- `"yes A"` / `"do it in 1098 and abandon 1099"`

### Live probes on `agy 1.5.17` (2026-11-25)

1. **Global sidecar route ruled out on `agy` CLI 1.5.17:** enabled a global sidecar in `~/.gemini/config/sidecars/` + `~/.gemini/config/config.json` and ran `agy --log-file`. `agy` CLI ran the turn and loaded hooks, and never started a sidecar manager (matching `sidecars.md`: "Available on: Antigravity App").
2. **Held-terminal wake via bracketed paste (`ESC[200~ … ESC[201~\r`) works live (4.8s):** sent to an idle `agy 1.5.17` PTY after `Stop`, fired `PreInvocation`, answered `WOKEN`, and fired `Stop`.
3. **Every unsafe state is distinguishable directly from `Hosted`'s existing `ghostty_vt::Terminal` (`text()`, `cursor()`, `alternate_screen()`, plus `bracketed_paste()` via `DATA_MODE` `2004`):**
   - **Trust dialog:** no `───` prompt box (`Do you trust…` / `↑/↓ Navigate · enter Confirm`), no `? for shortcuts`.
   - **Slash menu (`/`):** `> /` with `↑/↓ Navigate · enter Select · tab Complete`.
   - **Model picker (`/model`):** no `───` prompt box, `Keyboard: ↑/↓ Navigate … esc Go Back`, footer shows `esc to cancel`.
   - **Running turn:** footer shows `esc to cancel`.
   - **Typed draft:** non-placeholder text on the `>` line inside the `───` box (survives server restarts/adoption without depending solely on keystroke history).
   - **Empty idle prompt:** `───` / `>` (empty or placeholder hint only — verify dim/faint style or exact placeholder check and cursor position on `agy 1.5.16`/`1.5.17`) / `───` with `? for shortcuts` below, no `esc to cancel`, `!alternate_screen()`, `bracketed_paste()` on, `yielded == true`, `!pending_input.in_paste()`, and no recent user keystrokes.

### Built & checked (Pass 4)

1. **`loopflow-terminal` (`ghostty_vt.rs`):** exposed `Terminal::bracketed_paste(&self) -> bool` (DEC private mode 2004 via `DATA_MODE`) and made `Terminal::text()` and `Terminal::cursor()` available in non-test builds.
2. **`IdleReach::Terminal(TerminalId)` & opening idle recording:**
   - `IdleEndpoints` carries `held_terminal: Option<TerminalId>`; `sqlite/steers.rs::idle_conversation_candidates` reads `s.held_terminal_id` and allows `(s.provider_thread IS NOT NULL OR s.held_terminal_id IS NOT NULL)` while keeping `INDEXED BY agent_sessions_unended_agent`.
   - `Antigravity::idle_reach` returns `endpoints.held_terminal.clone().map(IdleReach::Terminal)` (removed `TODO(LOO-1099)`).
   - `Antigravity::open` records `record_first_boundary` (best-effort) when a managed launch opens without `-i`, so freshly opened or reopened idle `agy` work sessions without waiting steers can still wake when a steer arrives later.
   - `lf/commands/context.rs::read_news` checks `!prompt` before deferring `SessionStart` steers to a server wake, so Antigravity's `PreInvocation` (which is both start and opening prompt) always claims its steers.
   - Shared `loopflow_execution::steer::WAKE_PROMPT` across reopen (`native_session.rs`) and idle wake (`server/wake.rs`).
3. **VT-guarded held-terminal wake (`harness/antigravity.rs`, `transport/{input,hosted,registry}.rs`, `server/{wake,boundary,service}.rs`):**
   - `antigravity::prompt_ready` requires `!alternate_screen()`, `bracketed_paste()`, `at_ground()`, no live `Working`/`Blocked` OSC 7501 status, exact single-line empty prompt box (`───` / `>` / `───`), `? for shortcuts` in footer, and no `esc to cancel`, `↑/↓ Navigate`, `esc Go Back`, `enter Confirm`, or `enter Select`.
   - `transport::input::has_user_keystroke` ignores terminal query replies, focus reports, and SGR/X10/URXVT mouse events while holding off wakes for 2 s after real user keystrokes (`USER_KEY_QUIET`), open bracketed pastes (`PendingInput::in_paste()`), queued input, or post-restart ring replay.
   - `Registry::wake_idle_prompt` executes on the `held-terminals` thread and injects `ESC[200~ <WAKE_PROMPT> ESC[201~\r`; `server::wake` leaves steers unclaimed/unstamped for `IdleReach::Terminal` (Option A) and calls `store.record_woken(adapter)`. `PreInvocation` claims and stamps steers once `agy` starts the turn.
   - Busy/not-ready held terminals set `deferred` so `Watcher` retries every 2 s (`DEFERRED_RETRY`) without requiring a new store revision; `boundary::wake` also gives post-`Stop` TUI redraws two brief 300 ms retries before nudging `Watcher`.
4. **Tests & docs updated:**
   - Removed `TODO(LOO-1099)` from `server::wake::tests::every_harness_wakes_its_idle_work_session` — all 5 harnesses in `HarnessKind::ALL` pass idle wake unconditionally (Antigravity through a real holder + `Registry`).
   - Added `antigravity::tests::prompt_ready_accepts_only_empty_idle_prompt`, `transport::input::tests::mouse_focus_and_query_reports_are_not_user_keystrokes`, `transport::registry::tests::wake_idle_prompt_guards_against_drafts_typing_and_open_pastes_but_ignores_mouse_and_focus`, and `server::wake::tests::an_open_idle_antigravity_with_a_draft_defers_wake_until_the_prompt_is_empty`.
   - Fixed `wake.rs` test helper to use `test_env_lock() + test_ambient::EnvGuard` instead of `EnvGuard::with_lf_home` (which spawned a nested `cargo build --bin lf` inside unit tests).
   - Fixed `lf::commands::context::tests::a_wakeable_start_leaves_waiting_steers_to_the_wake` to set `provider='opencode'` for its `Inbox::Socket` fixture and cover `prompt == true`.
   - Updated `docs/sessions.md`, `rust/loopflow-execution/src/harness/README.md`, `rust/loopflow/src/server/README.md`, and `scratch/5whys.md`.

### Focused checks run (Pass 4)

- `cargo fmt --check` — passed
- `cargo test -p loopflow-terminal --lib ghostty_vt::tests::` — 8 passed
- `cargo test -p loopflow-storage --lib query_plans::` — passed
- `cargo test -p loopflow-execution --lib harness::antigravity::` — 8 passed (1 ignored live test)
- `cargo test -p loopflow-execution --lib transport::input::` — 7 passed
- `cargo test -p loopflow-execution --lib transport::registry::tests::wake_idle_prompt_` — 1 passed
- `cargo test -p loopflow --lib server::wake::tests::` — 14 passed
- `cargo test -p loopflow --lib server::boundary::tests::` — 14 passed
- `cargo test -p loopflow --lib lf::commands::context::tests::a_wakeable_start_leaves_waiting_steers_to_the_wake` — 1 passed

Deferred to gate / CI (high local Machine load ~100+): `cargo clippy --all-targets -- -D warnings`, `documented_commands::`, full acceptance matrix, and live QA on real `agy 1.5.17` (open idle wake, Task update / interactive-step handoff, and draft/menu non-clobbering).

## Remaining

- **Live QA on real `agy 1.5.17` (qa / gate):** demo 1 end to end — open idle wake from `lf steer`, a Graph settling at an interactive step, and a draft, `/` menu or model picker left untouched until the prompt is empty. No live run of the Pass 4 path exists yet; only unit fixtures and the 2026-11-25 probes.
- Clippy, `documented_commands::` and the acceptance list below were not run locally (Machine load); gate runs them.

## Acceptance at gate

```sh
cargo test -p loopflow --test integration documented_commands::
cargo test -p loopflow-terminal --lib ghostty_vt::tests::
cargo test -p loopflow-storage --lib query_plans::
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
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

## Outside this work

- Inline steer & Task-update display in Antigravity TUI: **LOO-1103** (`task_a265825e5d3e484b88cd7c047a79b675`, waiting on a working display-only `agy` hook field).
- Viewer handoff (multi-viewer `agy`): **LOO-1097**.
- Headless unanswered questions on Claude, Codex, Antigravity: **Task 48edb9**.
- Claude cross-session steer framing (`scratch/qa.html` pass 2 #1: Claude `task-session` primaries acknowledge `<cross-session-message>` steers and wait for typed confirmation while Codex and Antigravity act immediately): separate prompt/framing outcome outside this diff.
