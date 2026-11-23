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

Open-time lines such as "This conversation has no saved claude transcript…"
are on screen in the view, above the provider's first output. Save the PTY's
raw bytes and render them (`uv run --no-project --with pyte python`, a
`pyte.HistoryScreen(120, 40)` fed through `pyte.ByteStream`) to see what a
person sees. When stripping escapes by regex, end an OSC at BEL or `ESC \`:
lf's and Claude's OSCs end in `ESC \`, and a BEL-only pattern deletes the
text up to the next BEL, open-time lines included.

For a provider that refuses a saved thread, have the stand-in write the
transcript under `$CLAUDE_CONFIG_DIR/projects/<project>/<id>.jsonl` on
`--session-id` and exit 1 on `--resume`. To exercise this with a Task, create
and check out a disposable Task in a [private Machine](served-launches.md),
end the stand-in, and run `lf resume --task <id>`.

Opening a conversation in a private home registers a launchd job
(`com.loopflow.server.<hash>`, plist in the home) that restarts `lf server
run` when you kill it. To stop it, read the PID's `LF_HOME` with `ps eww`,
find its label in `launchctl list`, and
`launchctl bootout gui/$(id -u)/<label>`; then kill the home's `lf holder
run` and the providers it holds (a held `claude` outlives its view). Delete
the scratch repositories and their `~/.claude/projects` folders.

A headless turn (`lf -b steer <id> …`) does not take this
path: it resumes the recorded thread whether or not a transcript exists.
