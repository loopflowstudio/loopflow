# LOO-1097: hold interactive chats when the server is running

Brief, verbatim: "whenever i open an agy session in a different viewer Eg Rex
or Loopflow Desktop it seems to close thea gy client... is agy not being held
in the the holder the same way as other clients"

## Current implementation

Fresh default and inline chats, including `lf -a agy`, and bare terminal
resumes use the running Machine server. A new chat registers its Session when
the holder accepts the provider's exit record. A replay starts in the original
invocation directory so relative `--cwd` and `--repo` still resolve. The held
child performs default-agent worktree placement once. Starting the replay in
the already selected directory produced `Error: enter subdir: No such file or
directory (os error 2)` in the PTY check with `--cwd subdir`. A fresh Desktop
pane connects by terminal ID while its child creates the Session.

`cargo test -p loopflow --test transport_tests direct_chat_` passed all three
PTY checks with an agy stand-in. Two viewers submitted one shared draft, and
the same provider answered after the Machine server restarted. The no-server
and fresh Desktop pane checks passed. `cargo fmt` and
`cargo clippy --all-targets -- -D warnings` passed. Gate owns native UI checks.

Two parts remain before the feature ships.

- **Standalone Skills.** The proposed blanket delegation bypasses
  `with_skill_runtime`'s completion event and `commit_skill_step` in
  `bin/lf.rs`. A successful native exec never returns there. Keep the Skill
  launcher responsible for completion after the last view detaches, or hand
  that responsibility to the holder. Exercise a Skill that writes a file,
  detach, then check its commit and its completion event. Direct Graph steps
  continue under their Graph's launcher.
- **Idle retirement.** Implement pass 2 before release. In the current schema
  use `primary_scope_sql` for Task, Wave and repository primaries. The
  terminal host accepts a close notice unconditionally, so also check live
  viewers at the point of retirement. A delayed store observation alone cannot
  authorize closing a newly viewed terminal.

## What is happening

agy is wired into the holder the same way as the other four harnesses. What is
not held is a **chat**, on any harness: `lf -a agy` or `lf <skill>` typed in a
terminal runs directly in that pane (`os_process::foreground`, #1877 / #1966)
instead of in `lf holder run`. Opening a running chat anywhere else hits
`SessionResumeIntent::TakeOver` (`ops/human_session.rs` `resume_intent`),
which calls `stop_session_agent` (`SIGTERM`, then `SIGKILL`) before restarting
the provider from history. Any active turn is aborted, unsent input is lost,
and the first pane prints `"This conversation moved to Desktop: lf resume …"`
(`native_session.rs:516`) and drops back to the shell.

agy hits it most because of how it gets launched. Shell history shows
`lf -a agy` and `lf resume -a agy`. In a linked checkout that no Task owns
(`manabot.mlx`, `etude.demo-googlecolab`), `lf resume` picks
`checkout_conversation`, which is the last chat, and `transport::session_kind`
reopens a bare Session (`SqliteStore::is_bare`) as an unheld `Chat` again.
What the store showed on 2026-11-24 (`loopflow.db`, interactive agent
processes, last 4 days):

| provider | chats | chats ended with no exit recorded | Task work sessions |
|---|---|---|---|
| agy | 38 | 25 | 76 |
| claude | 14 | 6 | 72 |
| codex | 6 | 6 | 0 |

Live then: two agy chats in Rex panes (`session_7f28…`, `session_e288…`,
terminal `rex:{…}`), both showing the Desktop action **Take over**. One Claude
chat in Rex showed the same action. Held agy work sessions (`89aa1683…`,
`a9e69401…`) showed **Open here** and stayed up.

## Design (choice made for review; see Passed over)

**One interactive Session model: every interactive `lf` conversation runs in
the holder when the Machine server is running, with automatic 15-minute
idle-unviewed retirement for standalone chats and automatic direct-terminal
fallback when no server is running.**

Hosting (shared held PTY vs direct TTY) is separated from unviewed lifetime
(scope-owned vs standalone):

1. **Direct interactive launches run in the holder when the server is up.**
   In `rust/loopflow/src/bin/lf.rs` (for default `lf`, `lf -a <agent>`, inline
   prompts, and Skills — after resolving the target so unknown skills and bad
   flags still fail immediately in the caller, and not for Graphs, which spawn
   their own steps), when `LaunchInput::mode() == Mode::Interactive`,
   `transport::current().is_none()`, not detached, not `cfg!(test)`, both
   `stdin` and `stdout` are terminals, and the Machine server accepts a
   connection (`Connection::open().is_ok()`), delegate the invocation to
   `transport::start_held` and exec into the relay (`relay_with_preview`).
   - Inside the holder, `transport::current().is_some()`, so
     `LaunchInput::kind()` returns `LaunchKind::WorkSession`, hooks and
     capabilities run managed, and `native_session::run_client` hands its exit
     record to the server and `exec_native`s directly into the provider — zero
     extra `lf` process beside the agent.
   - Because the exact `argv` runs inside the holder, all flags (`-a`,
     `--yolo`, `--chrome`, `-c`, `--task`, `--wave`, inline prompt, skill) and
     `print_context_header` carry across unchanged, and Desktop sees the
     context header in scrollback too.
   - Replay the command in its original directory. The child resolves
     `move_default_agent_to_worktree` and runs the provider in that worktree.
   - When the Machine server is NOT reachable (`Connection::open()` fails in
     <1 ms — containers, CI, `lf server stop`, isolated test homes without a
     server), do NOT spawn a server and do NOT fail: run directly in the
     caller's terminal as `LaunchKind::Chat` via `os_process::foreground`.

2. **Bare `lf resume` also runs in the holder when the server is up.**
   In `transport::session_kind(id)` (`rust/loopflow-execution/src/transport.rs`),
   a bare Session (`store.is_bare(&session) == true`) resolves to
   `LaunchKind::WorkSession` whenever the Machine server is reachable
   (`Connection::open().is_ok()`), falling back to `LaunchKind::Chat` only
   when the server is unreachable.
   - Bound/primary Sessions and relayed viewers (`LF_SERVER_STATUS`,
     `LF_VIEWER_STREAM`) keep requiring the server and returning
     `LaunchKind::WorkSession` unconditionally.
   - Consequently, opening any running held Session from Desktop, another Rex
     pane, cmux, tmux, or phone always resolves to `SessionResumeIntent::View`
     (**Open here**): lossless attach, every view types, active turns and
     unsent drafts survive untouched, zero cold restart.
   - If a rare unheld Session exists (started while the server was stopped, or
     from an earlier build), `resume_intent` still returns
     `SessionResumeIntent::TakeOver` with its honest **Take over** label, and
     taking it over while the server is running lands it in the holder so
     subsequent opens are `View`.

3. **The server's `Registry` learns dynamically created Session IDs with zero
   protocol or schema change.**
   When `start_held` launches a new direct command (`lf -a agy`, `lf <skill>`),
   `Start.session` is `None` because the `LfSession` is created inside the
   held child. Right before `exec_native` / `exec_client`, every harness
   already sends `Request::ExitRecord { key, pid, record }` over `held.sock`,
   carrying `record.session` (`NativeExit.session`) and persisting
   `exit-record.json`.
   - In `transport/registry.rs` and `transport/hosted.rs`, when
     `Request::ExitRecord` arrives for a terminal whose `Registry::live` entry
     is `None`, bind `live[terminal] = Some(LfSessionId::from(&record.session))`
     and update `<terminal>/meta.json` (and on server restart in `take_up`,
     fall back to `exit-record.json`'s `session` if `meta.session` is `None`).
   - Result: `Registry::session_terminal(&session)` finds newly launched
     direct chats for Desktop `session.attach`, `lf open`, `server/wake.rs`,
     and after `lf install` server restarts — with no `held.sock` revision bump
     and no SQLite migration.

4. **Standalone held chats retire after 15 minutes unviewed when idle.**
   Telemetry showed 37 of 58 chats over 4 days ended via closed terminal tabs
   without `/exit`. Keeping unviewed standalone chats alive forever would leak
   idle provider processes and PTYs.
   - Reuse the existing `held_terminals.viewer_hold` / `viewer_hold_at` +
     `WORKSPACE_GRACE` (15 minutes) + `terminal_retention` (active turn or
     pending request holds it open) + `transport::close_notice` machinery
     already built for completed Tasks (`ops/task/retirement.rs`,
     `sqlite/task_retirement.rs`).
   - Add a storage query for due unviewed standalone held terminals:
     `held_terminals.ended_at IS NULL`, joined to an interactive
     `agent_sessions` row where `bound_at IS NULL` and not in
     `primary_sessions`, whose `viewer_hold` is `Absent(at)` (or stale
     `Viewed(at)` past `VIEWER_OBSERVATION_GRACE`, or `NULL` using
     `created_at`) with `at + WORKSPACE_GRACE <= now`.
   - Before closing each candidate, confirm `store.terminal_retention(&terminal)`
     is `None` (never interrupt an active turn or pending request), then call
     `transport::close_notice(&terminal, "Session closed after 15 minutes without a viewer")`.
   - Run this helper from both `ops/task/retirement.rs` (`lf task retire`) and
     `server/recovery.rs`'s 1-minute watcher pass so it works on dev Machines
     and test servers without launchd cron too.
   - Binding a running standalone chat (`lf session bind <id> --task …`) sets
     `bound_at`, automatically moving it onto open-Task lifetime with no
     restart.
   - Open Task, Wave, and Repository conversations keep their existing
     lifetimes unchanged; `server/recovery.rs` still auto-restarts only open
     Tasks' primary conversations.

5. **Words and docs stay honest.**
   - Keep `SessionResumeIntent::TakeOver` and its label **Take over** (never
     disguise a destructive process kill as **Open here**).
   - Update `docs/glossary.md`, `docs/sessions.md`, `docs/dev/index.md`, and
     `rust/loopflow-execution/src/transport/README.md` to match the real
     behavior: interactive `lf` launches run in the holder when the server is
     running (standalone chats retire after 15 minutes unviewed when idle;
     direct foreground `Chat` is the no-server fallback).

### Sibling harnesses

All five harnesses (Claude, Codex, OpenCode, Pi, Antigravity) share the same
delegation seam, `native_session::run_client`, `Request::ExitRecord`, and
`LaunchKind::WorkSession` path (already exercised across all five in
`harness/launch_matrix.rs`). Check during implementation:
- **Codex:** as a held `WorkSession`, `codex.rs` starts its private app-server
  and `exec_client`s into `codex --remote unix://…`, sending `ExitRecord` with
  `client: true` and `session` before exec — verify `Registry` binds `session`
  for Codex remote clients too.
- **Antigravity (`agy`):** sends `ExitRecord` before `exec_native`; hooks write
  `reported_thread` and `a_launch_follows_the_conversation_a_person_is_in`
  already covers resumed threads in the holder.
- **Claude / OpenCode / Pi:** all send `ExitRecord` before `exec_native` in
  `native_session.rs:588`.

## Demo

1. With `lf server` running, open Rex pane 1 in a scratch worktree and run
   `lf -a agy`, then send a message (even while `agy` is mid-turn or has an
   unsent draft typed).
2. Open that Session in Loopflow Desktop (**Open here**), or run
   `lf resume SESSION` in Rex pane 2.
3. Both viewers show the exact same live `agy` process. Nothing closed or
   restarted; mid-turn output and unsent drafts stay intact; both viewers type.
4. `lf session list` shows one running `agy` Session with a held terminal.
   Run `lf install` (or restart `lf server`) and confirm both viewers reconnect
   without interrupting `agy`.
5. Close both viewers without `/exit`: `lf session list` shows `detached`.
   Reattach within 15 minutes and it returns live; leave it unviewed past 15
   minutes (idle) and the cleanup pass closes the held terminal cleanly while
   `lf resume SESSION` still resumes from saved history.
6. Stop `lf server` (or use an isolated `LF_HOME` with no server) and run
   `lf -a agy`: it runs directly in the terminal without error.

## Acceptance checks at gate

1. **Direct interactive launch held + lossless second view (integration test
   with `MachineServer` and a stand-in provider):**
   - Start a direct `lf` interactive launch in PTY 1 against a running
     `MachineServer`.
   - Confirm `session_terminal` is populated in SQLite AND
     `Registry::session_terminal` answers `SessionAttach` for that Session ID.
   - Open the Session from PTY 2 via `lf resume SESSION` (and verify
     `resume_intent` is `View`, label is **Open here**).
   - Assert PTY 1's provider PID never changed (no `SIGTERM`/restart), both
     PTY 1 and PTY 2 receive live output and can write input, and PTY 1 never
     prints `"This conversation moved to"`.
2. **Server restart continuity:** restart `MachineServer` against the same
   holder and assert `SessionAttach` still resolves the dynamically created
   Session via `meta.json` / `exit-record.json`.
3. **No-server fallback:** run direct `lf` in a PTY with no `MachineServer`
   running and assert it runs directly in the foreground without error.
4. **Standalone 15-minute idle-unviewed retirement (unit/storage + integration
   test):**
   - Bare held terminal with `Absent(now - 16m)` and no `terminal_retention` is
     closed by the retirement pass.
   - Bare held terminal with `Absent(now - 5m)`, or `Viewed`, or active
     `terminal_retention`, or bound to a Task / primary scope is NOT closed.

## Absent and error states

- **Server not running on direct `lf` or bare `lf resume`:** `Connection::open()`
  fails immediately; fall back cleanly to foreground `LaunchKind::Chat`. Never
  fail a direct launch because `lf server` is down.
- **Server running, `start_held` fails mid-call** (e.g., server died between
  probe and `SessionStart`, or PTY limit reached): for direct launches, log a
  warning and fall back to foreground `Chat` (or surface the PTY-limit error if
  the server explicitly answered an error).
- **Store unreadable during retirement pass:** store failures never authorize
  closing a held terminal (same rule as Task retirement).
- **Piped stdin (`!stdin.is_terminal()` or `!stdout.is_terminal()`):** skip
  direct-launch delegation so piped prompts and non-TTY callers keep working
  unchanged.
- **Inside a holder or detached process (`transport::current().is_some()` or
  `detached::launched()`):** never double-delegate. Note: running `lf -a agy`
  inside a Desktop Task shell has `held_in().is_some()` and
  `current().is_none()`, so it properly starts its own held terminal and relays
  it inside the shell.

## Exclusions

- No new CLI flags (no `--direct` / `--no-holder`). Plain `agy` / `claude`
  already exists for raw unheld execution.
- No change to headless `Batch` (`lf -b`) or `Worker` (`lf task run`) launches.
- No change to Task/Wave/Repository session lifetimes or auto-recovery rules.
- No SQLite schema migration and no `held.sock` protocol revision bump.
- No fragile SIGKILL-follower choreography for the rare unheld `TakeOver` case;
  rare unheld sessions keep their honest **Take over** label and move notice.

## What goes

- The rule that direct interactive `lf` launches always bypass the holder even
  when `lf server` is running.
- The rule in `transport::session_kind` that bare `lf resume` in a terminal
  drops back to unheld `Chat` while the server is running (and the behavioral
  gap between Rex and Desktop).
- Outdated prose in `docs/glossary.md`, `docs/sessions.md`, `docs/dev/index.md`,
  and `rust/loopflow-execution/src/transport/README.md` describing direct `lf`
  as always unheld and opening a running chat as a take-over.

## Passes

1. **Hold direct launches & bind dynamic Sessions in `Registry`.** Default, inline and bare resume implemented. Standalone Skills require the completion work above.
   - Update `transport/hosted.rs` + `transport/registry.rs` so
     `Request::ExitRecord` binds `Registry::live[terminal] = Some(session)` and
     persists `meta.session` (with `exit-record.json` fallback on server
     restart).
   - Add `transport::delegate_launch` (probing `Connection::open().is_ok()` on
     interactive TTY launches) and wire it in `bin/lf.rs` for direct
     interactive launches (default `lf`, inline, Skill — after target
     resolution and `move_default_agent_to_worktree`), plus update
     `transport::session_kind` so bare `lf resume` also uses the holder when
     the server is reachable.
   - Build the `MachineServer` integration tests first (direct launch held +
     lossless second view without PID restart, server restart continuity,
     no-server fallback) and update affected `transport.rs` unit tests.
2. **Standalone 15-minute idle-unviewed retirement.**
   - Add the bare unviewed-terminal query in `loopflow-storage`
     (`sqlite/task_retirement.rs`) and `close_unviewed_chats` in
     `ops/task/retirement.rs` (guarding on `terminal_retention` and
     `VIEWER_OBSERVATION_GRACE`), invoked from both `lf task retire` and
     `server/recovery.rs`'s 1-minute pass.
   - Add unit + integration tests covering due bare retirement, active-turn
     protection, bound/primary exclusion, and viewer return canceling closure.
3. **Docs & consistency sweep.**
   - User and architecture docs drafted ahead of implementation in:
     - `docs/sessions.md`
     - `docs/glossary.md`
     - `docs/server.md`
     - `docs/dev/index.md` (and rendered `docs/dev/index.html`)
     - `rust/loopflow-execution/src/harness/README.md`
     - `rust/loopflow-execution/src/transport/README.md`
     - `rust/loopflow-execution/src/harness/README.md`
     - `rust/loopflow/src/server/README.md`
   - Verify every reference matches the shipped implementation (especially exact close-notice text and direct `lf graph` interactive-step behavior: confirm whether interactive steps spawned by a direct terminal `lf graph` inherit held delegation or stay in the caller's terminal).

## Passed over

- **Promote-on-second-open + follower attaches** (earlier kickoff plan). Still
  calls `stop_session_agent` (`SIGTERM`/`SIGKILL`) on first cross-viewer open,
  aborting active turns and wiping drafts; hides that kill under the **Open
  here** label; flips a Session back to unheld on the next Rex `lf resume`;
  leaves raw terminal modes unhandled after `SIGKILL`; and adds lock-wait
  follower complexity on top of the broken split.
- **Hold standalone chats indefinitely with no unviewed retirement.** Telemetry
  (`loopflow.db`, 37/58 chats closed without `/exit` in 4 days) shows closed
  tabs would pile up idle provider processes and exhaust PTYs.
- **Hard-require `lf server` for direct `lf` launches.** Breaks bare CLI usage
  in containers, CI, and stopped-server environments and violates the rule that
  `lf` never refuses to launch a provider when its own server is down.
