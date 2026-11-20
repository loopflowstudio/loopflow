Read when the change touches how a conversation reopens: resuming its provider thread or starting a new one.

# Reopening a conversation

Drive `lf -a claude resume` from a Python `pty.fork()` in a fresh Git
repository with a private `LF_HOME`, the real `HOME`, and `LF_*`,
`CLAUDE_CODE*`, `CLAUDECODE`, `REX_*` and `LOOPFLOW_*` stripped. The view
attaches to a held terminal, so the provider is not the PTY child's
descendant. Read what launched from the private store with Python `sqlite3`
(`file:…/loopflow.db?mode=ro`): `agent_sessions.agent_command` ends in
`--session-id <uuid> --` for a new thread or `--resume <uuid>` for a saved
one, and each `session_events` row with `kind='captured'` is one input with
its `provider_thread`. A saved thread is
`~/.claude/projects/<cwd with / and . as ->/<uuid>.jsonl`; the cwd is the
`<repo>.repo` checkout beside the repository.

A repository or Task conversation starts with lf's own first turn, so real
Claude saves its transcript as soon as it gets past its first screen. The
only quit-before-typing point is that screen: the `--add-dir` dialog
("Security guide … No, exit / Yes, add it"), shown on every launch in a new
directory. Esc or Enter on "No, exit" exits 1 with nothing saved. To answer
yes send `ESC O B` (application cursor Down), then `\r`. Keys sent in a
view's first few seconds are sometimes lost; a second view of the same held
terminal (`lf -a claude resume` again) answers it. A parent directory's trust
does not cover the dialog.

For a provider that dies before saving, put a stand-in `claude` first on the
private `PATH` that echoes its `--session-id`/`--resume` argument and exits 1.

Lines the held terminal's own `lf` prints before the view attaches (the
open-time diagnostics in `ops/human_session.rs`) do not reach the view; they
are in `$LF_HOME/screens/*.ghostsnp` (`strings`).

Stop the private home's `lf server run` and `lf holder run` by PID after
checking `ps eww` shows its `LF_HOME`; a held `claude` outlives its view.
Delete the scratch repositories and their `~/.claude/projects` folders.
