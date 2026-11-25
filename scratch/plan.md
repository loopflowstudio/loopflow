# LOO-1097: hold interactive chats when the server is running

Brief, verbatim: "whenever i open an agy session in a different viewer Eg Rex
or Loopflow Desktop it seems to close thea gy client... is agy not being held
in the the holder the same way as other clients"

## Pass 1 — shared viewers (`979b150c16`)

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

## Pass 2 — Session endings

- Both the native launcher and the holder record the observed ending against
  the agent generation. Exit 0 or 130 shows `quit`. Intentional retirement
  shows `closed HH:MM (<reason>)` with the resume command and stays down.
  Other exits retain the provider's code. A new attachment clears the old
  ending. Historical ends without a reason keep their previous reading.
- The host writes the reason beside its terminal before retirement. A
  delegated Skill hands over its capture and attachment in `bind_session`
  for this case. On ordinary return the launcher still owns its checkpoint.
  Recovery reads the same receipts when retirement spans a server restart.
- At 10,000 Tasks and 40,000 Sessions, recording the reason used
  `agent_sessions_agent_process` and took 5.8 µs median across 100 writes
  inside a private SQLite transaction. Replaying an ending changed zero rows.
  The recovery query kept its previous query plan and read 100 checked-out
  conversations in 0.39 ms median across 100 reads.

### Focused checks

- `transport_tests` PTY runs passed for fallback quits, detached Skill
  checkpoint and quit, chat retirement followed by a fallback crash, and
  Skill retirement across server restart. Both retired captures completed
  and both saved screens showed the close notice.
- `loopflow-execution` tests passed for Session outlook, Task recovery,
  replacement, provider failures, delayed recording, and settlement from a
  saved retirement.
- `uv run python scripts/schema_page.py --check` and
  `cargo clippy --all-targets -- -D warnings` passed.

## Remaining at gate

- Update `docs/sessions.md` lines 70–72 so it matches `HEAD`: scoped Task,
  Wave, and repository conversations stay in `lf session list` showing
  `closed HH:MM (Task completed)` / `closed HH:MM (Task abandoned)` and
  `quit HH:MM`, whereas standalone chats complete (`completed_at` set) on
  both `/exit` and 15-minute idle retirement and leave `lf session list`
  (inspectable via `lf session card <id>` / `lf resume <id> --prepare`,
  findable via `lf history search`, with the close notice on the saved screen
  when reopened).
- Open an actual agy conversation in Rex, type a draft, then join from Desktop.
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
