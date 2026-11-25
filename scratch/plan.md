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

### Implementation plan (Pass 4)

1. **Expose `Terminal::bracketed_paste(&self) -> bool`** on `loopflow-terminal`'s `ghostty_vt::Terminal` (using `DATA_MODE` + `ModeConfig` for mode `2004`, plus any cell-style/cursor helper needed to distinguish dim placeholder text from a typed draft cleanly).
2. **Add `IdleReach::Terminal(TerminalId)` (and `held_terminal_id` on `IdleEndpoints`):**
   - `sqlite/steers.rs::idle_conversation_candidates` includes `held_terminal_id` (and allows candidates whose `held_terminal_id IS NOT NULL`, keeping the existing index plan).
   - `Antigravity::idle_reach` returns `endpoints.held_terminal.map(IdleReach::Terminal)` — removing `TODO(LOO-1099)`.
   - Preserve pass-through when `MachineServer` or the store is unavailable.
3. **VT-guarded held-terminal wake in `transport::hosted` / `transport::registry` + `server::wake`:**
   - Filter terminal query replies (DA, CPR, theme, focus) and SGR/X10 mouse events from user keystrokes so focus/scroll never blocks a wake, while real keystrokes hold off wakes briefly (`last_user_key` / `unsubmitted_input`).
   - Before waking an `IdleReach::Terminal` conversation, check the `Hosted` terminal's VT screen readiness + keystroke guard: `bracketed_paste()`, `!alternate_screen()`, `!in_paste()`, quiet since last user keystroke, and VT screen shows `agy`'s empty top-level prompt box (not trust dialog, not slash menu, not model picker, not `esc to cancel`, no typed draft — so server restarts/adoptions also protect drafts).
   - **Option A payload:** do **not** claim/stamp steers in `server::wake` for `IdleReach::Terminal`; instead verify `has_undelivered_steers` (after `task_updates::write`), check terminal readiness, inject bracketed paste `ESC[200~ Read the waiting steers and Task updates for this Session. ESC[201~\r` (the same trigger prompt as reopen), and call `store.record_woken(adapter)` so the watcher sends no second paste while the turn starts. `PreInvocation` claims and stamps the steers/updates once `agy` actually processes the prompt, so a dropped paste loses nothing and never double-delivers.
   - If the terminal is not ready (user typing, modal open, trust dialog, etc.), leave steers untouched for the next turn or watcher tick. (Also re-check readiness when `Hosted` sees `?2004h` / prompt settle or on watcher ticks.)
4. **Tests & live QA:**
   - Remove the `TODO(LOO-1099)` exception from `server::wake::tests::every_harness_wakes_its_idle_work_session` so **all 5 harnesses in `HarnessKind::ALL`** pass idle wake unconditionally.
   - Add unit tests for the VT prompt-readiness predicate covering every captured state: empty prompt (blank and placeholder), typed draft (even with no prior keystroke history after restart), trust dialog, slash menu, model picker, running turn (`esc to cancel`), alternate screen, and mouse/focus events not counting as drafts.
   - Update `docs/sessions.md`, `rust/loopflow-execution/src/harness/README.md`, `rust/loopflow/src/server/README.md`, and `scratch/5whys.md` to reflect that already-open idle `agy` wake is implemented.
   - Verify live on real `agy 1.5.17` in QA: open `agy` Task session, let it go idle, run `lf steer <task> 'Reply with only the word OK.'` (and a Task update / interactive-step handoff), verify it wakes without typing and leaves `0 pending`, AND verify a typed draft / slash menu / model picker is NOT clobbered.

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
- Viewer handoff (multi-viewer `agy`): **LOO-1097**.
- Headless unanswered questions on Claude, Codex, Antigravity: **Task 48edb9**.
- Claude cross-session steer framing (`scratch/qa.html` pass 2 #1: Claude `task-session` primaries acknowledge `<cross-session-message>` steers and wait for typed confirmation while Codex and Antigravity act immediately): separate prompt/framing outcome outside this diff.
