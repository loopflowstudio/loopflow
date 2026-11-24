Read when the change reads raw provider conversations or their transcripts.

# Raw conversations and transcript readings

A plain `claude` lists as raw only with no `lf` or provider ancestor, and a
shell this step starts has one. Run it under a PTY driver that double-forks and
calls `setsid`, so launchd is its parent; keep the real `HOME`, since raw
discovery reads the account's own `~/.claude/projects`, and give lf a private
`LF_HOME`. Strip `LF_*` and `CLAUDE_CODE*` from its environment. Claude's
folder-trust dialog in a fresh directory defaults to "No, exit": send Down,
then Enter. It writes no transcript until the first message, so the
conversation lists only after one.

`lf <read> --subscribe` from the CLI reads in its own process, not through the
server (`lf server status` shows no client). To check what Desktop sees,
subscribe over the server socket with a `desktop` hello and timestamp each
`reading`; compare its frame times with the transcript rows' timestamps.

A bare `lf -a claude : "<prompt>"` in that private `LF_HOME` with no accounts
and the real `HOME` runs on the ambient Claude login, so its Stop-hook and
exit rows can be read from the private store. Unset `REX_*` too, or the run
records the agent's own Rex pane. Read the store with
`sqlite3 'file:<db>?immutable=1'` once no writer is open; `-readonly` fails
when the WAL files are gone. `~/.claude/sessions/<pid>.json` names each live
Claude process's conversation. Run a second `claude` and an lf run in the same
directory to check pairing. Codex's first-run trust prompt writes into the real
`~/.codex/config.toml`; quit it unless that write is acceptable.

Plain `pi` and `opencode` need no trust answer and list the same way. A
headless `lf -b -a pi : '<prompt>'` (or `opencode`) in the raw process's
directory checks pairing against an lf-written conversation and writes
one `history.db` `turns` row per turn with `cost_usd`. OpenCode's default headless model may fail
upstream; name one that answered interactively, `-a opencode:opencode/<model>`.
`pi --session <file>` resumes a recorded Pi conversation raw. To learn
which filesystem events a provider home really sends, a throwaway ignored
test holding a `notify` watcher on it shows them; FSEvents sent none for
OpenCode's `opencode.db-wal` writes.

# Inherited session markers (caller's Claude Code or Codex)

Claude 2.1.327 drops a transcript only when interactive: `claude -p` (every
`lf -b` batch) keeps it even with `CLAUDE_CODE_CHILD_SESSION` set, so a batch
can't show the bug. Run an interactive `lf -a claude : '<prompt>'` chat from
an agent shell in a PTY, with `LF_*`, `REX_*` and `LOOPFLOW_*` stripped but
the `CLAUDE_*`/`CODEX_*` markers kept, the real `HOME` and a private
`LF_HOME`. A fresh directory's trust dialog defaults to "No, exit": Down,
Enter. A kept marker shows "⚠ Transcript saving is off — inherited
CLAUDE_CODE_CHILD_SESSION marker" in the footer, no `.jsonl` appears, and
`lf session resume <session>` starts the conversation again ("This
conversation has no saved claude transcript…"). The provider thread is `agent_sessions.provider_thread` in the private
`loopflow.db` (Python `sqlite3` with `?mode=ro`; the sqlite3 CLI couldn't
open it here). `ps eww -o command= -p <claude pid>` shows the environment lf
handed the provider; walk the PTY child's descendants to find it.
