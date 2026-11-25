# LOO-1097: hold interactive chats when the server is running

Brief, verbatim: "whenever i open an agy session in a different viewer Eg Rex
or Loopflow Desktop it seems to close thea gy client... is agy not being held
in the the holder the same way as other clients"

## Implementation and checks

- Direct terminal chats, including default, inline, `lf -a agy` and bare
  resumes, use the running Machine server. Without a server they run in the
  terminal. Replaying from the invocation directory fixed `--cwd subdir`
  being applied twice. Three `direct_chat_` PTY tests covered an unfinished
  draft across two viewers, server restart, no-server fallback and a fresh
  Desktop pane in the first pass.
- A delegated Skill keeps its launcher in the held PTY. The launcher
  registers the new Session with the host, waits for the provider, then uses
  the existing checkpoint and Skill completion path. A parent-launched Skill
  still leaves the checkpoint with its caller. Direct Graph steps keep their
  Graph's launcher.
- The Machine's minute pass and unscoped `lf task retire` retire idle
  standalone chats after fifteen minutes unviewed. Both check Session scope
  and retained work. The host checks its live viewers and latest departure
  before it ends the terminal. On restart it keeps a saved absence and
  dates a lost connection from the last heartbeat.
- The candidate query took 0.211 ms median over 100 reads with 10,000 Tasks,
  40,000 Sessions, 300 unfinished agents and 100 due terminals. SQLite used
  the unfinished-agent index and primary-key lookups on terminals and Task
  primaries.
- The private-Machine PTY checks exercised a file-writing Skill using two
  viewers, detaching both before provider exit, then reading the committed
  file and the Skill completion event. Retirement checks cover a late close
  request, recent detach, a pending turn, and the saved deadline on restart.

| Local check | Result |
| --- | --- |
| `cargo test -p loopflow-storage --lib unviewed_chat_candidates_follow_grace_and_current_scope` | Passed |
| `cargo test -p loopflow --test transport_tests standalone_` | Three passed on private Machines |
| `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` | Passed |
| `uv run python ../scripts/render_architecture_html.py --check` from `website/` | Passed |

## Remaining at gate

Open an actual agy conversation in Rex, type a draft, then join from Desktop.
Submit the draft from the second view, close the views, and resume the
conversation. Check both panes' native input and their reconnect after a
Machine server restart. Gate checks the documentation examples with
`documented_commands::`. CI owns the wider harness matrix.

## Before the change

The original issue affected chats on all five harnesses: `lf -a agy` or `lf <skill>` typed in a
terminal ran directly in that pane (`os_process::foreground`, #1877 / #1966)
instead of in `lf holder run`. Opening a running chat anywhere else hit
`SessionResumeIntent::TakeOver` (`ops/human_session.rs` `resume_intent`),
which calls `stop_session_agent` (`SIGTERM`, then `SIGKILL`) before restarting
the provider from history. The active turn was aborted, unsent input was lost,
and the first pane printed `"This conversation moved to Desktop: lf resume …"`
(`native_session.rs:516`) and dropped back to the shell.

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
