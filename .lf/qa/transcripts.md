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
`usage` rows with `cost_usd`. OpenCode's default headless model may fail
upstream; name one that answered interactively, `-a opencode:opencode/<model>`.
`pi --session <file>` resumes a recorded Pi conversation raw. To learn
which filesystem events a provider home really sends, a throwaway ignored
test holding a `notify` watcher on it shows them; FSEvents sent none for
OpenCode's `opencode.db-wal` writes.
