# LOO-1097: hold interactive chats when the server is running

Brief, verbatim: "whenever i open an agy session in a different viewer Eg Rex
or Loopflow Desktop it seems to close thea gy client... is agy not being held
in the the holder the same way as other clients"

## Built and verified on HEAD (`979b150c16`)

- Direct terminal chats, including default, inline, `lf -a agy` and bare
  resumes, run in `lf holder run` whenever the Machine server is reachable,
  and fall back directly to the terminal when no server is running.
  Replaying from the invocation directory keeps relative `--cwd` paths
  correct. Cross-viewer drafts, server restarts, no-server fallback and
  Desktop status attachment are covered by `direct_chat_` PTY tests and
  real-`agy` QA (`scratch/qa.html`).
- A delegated Skill (`lf <skill>`) keeps its launcher in the held PTY via
  `BindSession` + `LF_SKILL_CHECKPOINT`, waits for the provider, and runs its
  checkpoint and Skill completion path even after viewers detach. Direct
  Graph steps keep their Graph's launcher.
- Both the Machine server's minute pass and unscoped `lf task retire` retire
  idle standalone chats after 15 minutes without a viewer, checking Session
  scope and `terminal_retention` (never interrupting active turns or pending
  requests). The host rechecks live views and latest departure before ending
  the terminal, and preserves a saved absence across server restarts.
- Candidate query benchmark: 0.211 ms median over 100 reads at 10,000 Tasks,
  40,000 Sessions, 300 unfinished agents and 100 due terminals.

## Next pass: honest Session endings (`quit` vs `closed` vs `died`)

Post-build concept review (`scratch/concept-review.html`) found that the
change makes interactive hosting and cross-viewer opening line up, but leaves
how interactive Sessions end conceptually broken across three everyday paths:

1. **Retired standalone chats (64% of real chats in telemetry) and retired
   done/abandoned Task conversations** show up as crashes: `hosted.rs` sets
   `CloseReason::Retired` and delivers the close notice, then `child.retire()`
   sends `SIGKILL` and drops the retirement reason before settlement.
   `native_exit::settle` treats `SIGKILL` as a failure: writes
   `agent_exit_code = 137`, capture `outcome = "failed"`, OSC 7501
   `State::Error`, appends `"<provider> exited with 137."` to the saved screen
   after the close notice, and renders `died HH:MM (exit 137)` in
   `lf session list` and Desktop `status_line`.
2. **Cleanly quit delegated Skills (`lf default`, `lf implement`) and
   no-server fallback Chats** read `died HH:MM (no exit recorded)`:
   `native_session::run_client` (`spawn_native` path) calls
   `native_exit::record_exit` and never writes `agent_exit_code`, so
   `quit_by_person(None)` is `false` even on exit 0 or 130.
3. **Retired delegated Skills** (`BindSession`, no `exit_record`) are killed
   alongside their provider before they can settle, so they stay `detached`
   until `reap_agent_processes` sweeps up to 10 seconds later and records
   `agent_exit_code = NULL` → `died HH:MM (no exit recorded)`.

### Experience and model

An ended interactive conversation has one of three honest outcomes (besides
being moved/replaced):

- **You quit it** (`agent_exit_code` in `0 | 130`, `agent_end_reason IS NULL`)
  → `quit HH:MM; \`lf resume …\` opens it` (`SessionOutlookKind::Quit`).
- **Loopflow closed it** (`agent_end_reason` set, e.g. `"15 minutes without a
  viewer"`, `"Task completed"`, `"Task abandoned"`; `agent_exit_code IS NULL`)
  → `closed HH:MM (<reason>); \`lf resume …\` opens it`
  (`SessionOutlookKind::WaitingForYou`, `outlook.reason = Some(reason)`). Not
  a failure: capture `outcome = "completed"`, OSC `State::Done`, no
  `"<provider> exited with 137."` appended to the saved screen, and
  `ops::recovery::server_start` never treats an intentional close as a death.
- **It died** (`agent_end_reason IS NULL`, exit code not `0 | 130` or `NULL`)
  → `died HH:MM (exit N)` or `died HH:MM (no exit recorded)`. Real crashes,
  external kills, OOM, holder deaths and reboots still record failure and
  trigger open-Task auto-recovery.

### Changes across layers (recommended slice order)

1. **Storage & additive migration (first slice — proves the schema and
   summary reading soonest):**
   - Run `uv run python scripts/new_migration.py agent_end_reason` to add
     nullable `agent_end_reason TEXT` to `agent_sessions` and update
     `rust/loopflow-storage/SCHEMA.md`.
   - Clear `agent_end_reason = NULL` in `attach_in`
     (`rust/loopflow-storage/src/sqlite/processes.rs`) beside
     `agent_ended_at = NULL, agent_exit_code = NULL` so a retired Session
     that later resumes unheld without a server cannot leak its old retirement
     reason on a subsequent crash.
   - Add storage helpers:
     - `record_agent_end_reason(&self, agent: &AgentProcessId, reason: &str)`
     - `record_terminal_retirement(&self, terminal: &TerminalId, reason: &str)`
       to stamp `agent_end_reason`, `agent_ended_at`, `agent_spawn_state =
       'exited'`, and clear `agent_pid`/`agent_os_started_at`/live attachment
       on any open `agent_sessions` row still pointing at `terminal` (covers
       delegated Skills that have no `exit_record`).
   - Read `a.agent_end_reason` into `AgentEnd { at, exit_code, reason }` in
     `rust/loopflow-storage/src/sqlite/sessions.rs` and
     `TaskConversation.end_reason` in `sqlite/session_recovery.rs`.

2. **Session ending & recovery (`ops/human_session.rs`, `ops/recovery.rs`):**
   - Add `end_reason: Option<&str>` (or `retired: bool`) to
     `DownConversation` so `server_start` returns `Left::Closed` (or
     `Left::Quit` equivalent — never `ServerStart::At`) when Loopflow
     intentionally closed the conversation.
   - Update `session_ending` so `end.reason = Some(reason)` formats
     `closed HH:MM (<reason>); \`<command>\` opens it` with
     `SessionOutlookKind::WaitingForYou`, `outlook.reason = Some(reason)`,
     and `ended = Some(SessionEnd { at, exit_code: None })`. Zero breaking
     DTO changes: CLI and Desktop already render `status_line` directly.
   - Add unit tests in `ops/human_session/tests.rs` and `server/recovery.rs`.

3. **Waited native launches (`harness/native_session.rs`):**
   - In `run_client` (`spawn_native` path), right after `record_exit(owner)`,
     record `agent_exit_code` (`0` on `status.success()`, `130` if
     `interrupted`, else `provider_exit_code(status)`). Cleanly quit delegated
     Skills and no-server fallback Chats now read `quit HH:MM` instead of
     `died HH:MM (no exit recorded)`.

4. **Held terminal retirement & settlement (`transport/hosted.rs`,
   `transport/registry.rs`, `harness/native_exit.rs`):**
   - Keep `retire_reason: Option<String>` on `Hosted` (`"15 minutes without a
     viewer"` for `Request::RetireChat`, while still printing `"Session closed
     after 15 minutes without a viewer"` to the terminal; `text` for
     `Request::CloseNotice { text }`). Persist it to `retire-reason.json`
     beside the terminal before `child.retire()` so a server crash mid-retire
     preserves it.
   - Pass `retire_reason` into `Settling::spawn` → `native_exit::settle` (and
     read `retire-reason.json` in `registry::settle` for crashes/restarts):
     when `Some(reason)`, settle capture and attachment as `"completed"`,
     return `Settled { code: 0, moved: false, lines: Vec::new() }` (no
     `"<provider> exited with 137."` line on the screen or saved snapshot),
     report OSC `State::Done`, and record `agent_end_reason = Some(reason)`
     instead of `agent_exit_code = 137`.
   - When a retired terminal exits with `exit_record == None` (delegated
     Skill via `BindSession`), call `record_terminal_retirement` in
     `registry::settle` before `end_terminal` so the Session closes
     immediately with `agent_end_reason` and does not wait for
     `reap_agent_processes`.
   - Cover with focused PTY tests in `rust/loopflow/tests/transport_tests.rs`:
     (a) retired standalone chat reads `closed HH:MM (15 minutes without a
     viewer)` with no `exited with 137.` on the saved screen; (b) cleanly
     quit delegated Skill reads `quit HH:MM`; (c) retired delegated Skill
     immediately reads `closed HH:MM (15 minutes without a viewer)`; (d)
     resuming a retired chat without a server and exiting non-zero clears
     `agent_end_reason` and reads `died HH:MM (exit N)`.

5. **Docs check:**
   - Verify `docs/sessions.md` and `rust/loopflow-execution/src/transport/README.md`
     match the `closed HH:MM (<reason>)` status line and retirement behavior.

## Remaining at gate

Open an actual agy conversation in Rex, type a draft, then join from Desktop.
Submit the draft from the second view, close the views, and resume the
conversation. Check both panes' native input and their reconnect after a
Machine server restart. Gate checks the documentation examples with
`documented_commands::`. CI owns the wider harness matrix.

## Before the change (reference)

The original issue affected chats on all five harnesses: `lf -a agy` or
`lf <skill>` typed in a terminal ran directly in that pane
(`os_process::foreground`, #1877 / #1966) instead of in `lf holder run`.
Opening a running chat anywhere else hit `SessionResumeIntent::TakeOver`,
which killed the provider (`SIGTERM`/`SIGKILL`) and dropped the first pane to
the shell. Store telemetry on 2026-11-24 (`loopflow.db`, 4 days) showed 37 of
58 interactive chats ended without `/exit` (tab close), confirming why
standalone held chats need 15-minute unviewed retirement AND honest closed
reporting rather than `died (exit 137)`.

## Passed over

- **Promote-on-second-open + follower attaches** (earlier kickoff plan). Still
  kills mid-turn on first cross-viewer open and disguises a kill as **Open
  here**.
- **Hold standalone chats indefinitely with no unviewed retirement.** Leaks
  idle provider processes and PTYs on closed tabs.
- **Hard-require `lf server` for direct `lf` launches.** Breaks bare CLI in
  containers, CI, and stopped-server environments.
- **Store retirement reason only on `held_terminals`.** Leaks a stale
  retirement reason if a retired Session is later resumed unheld without a
  server and crashes (because `attach_in` does not clear `held_terminal_id`).
- **Leave retired chats reading `died (exit 137)` as a follow-up.** Makes
  ~64% of everyday chats look crashed in `lf session list`, Desktop, saved
  screens, and captures.
