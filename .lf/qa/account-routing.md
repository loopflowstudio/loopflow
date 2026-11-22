Read when the change touches native provider logins, historical accounts or quota refusals.

# Native logins and provider refusals

lf selects no login: Claude and Codex read their own homes. To check that an
old managed account changes nothing, seed `provider_accounts` and
`provider_session_accounts` rows by raw SQL in the private store and put
released `account:`/`accounts:` keys in `$LF_HOME/config.yaml`; launches must
stay on the native login, print no "unknown setting" warning and leave those
rows and files byte-for-byte. `lf session list` then shows "historical
account: <id>". With `isolated=1` and the transcript only under the account's
`home/projects/`, a resume copies the transcript (never credentials) into the
native home; with no transcript anywhere it refuses with "its transcript is
missing". For a quota refusal, put a stand-in `claude` first on `PATH` that
prints `init`, a `rate_limit_event` with `status: rejected`, then an
`is_error` `result`, and configure `agents: [claude, codex]`: unpinned `lf -b`
prints "Continuing on codex" and real Codex answers; `-a claude:opus` stops
with "agent stopped after provider usage limit". A refused Codex for a real `lf`: copy `refused_codex_script`
(`rust/loopflow-execution/tests/support/mod.rs`) into a `codex` first on
`PATH`, keeping its `LF_TEST_CODEX_STDIO` guard (the bridge
`tests/support/codex_socket.py` runs the script again with it set; without the
guard the launch fails with "Address already in use"), answer `--version`, end
with `exit 0`, and point `CODEX_HOME` at a disposable directory, where it
writes its rollout. Swap `codexErrorInfo` and the message for a revoked login
(`unauthorized`, "refresh token was revoked"). When the account's own Codex is
out of quota (`grep -h 'hit your usage limit' ~/.lf/logs/*.log | tail -1`
names the reset time), skip the stand-in: a private `LF_HOME` with the real
`HOME` gets the live refusal and real Claude answers.

The order to test is `agents:` in `$LF_HOME/config.yaml`; a person's global
value outranks a committed `.lf/config.yaml`. Cover `lf -b : '<prompt>'`,
`lf commit`, `lf sync` against a conflicting `origin/main` (keep local `main`
at `origin/main`), and `lf pr open` with a `gh` stand-in on `PATH` that logs
its arguments, answers `pr list` with `[]` and `pr create` with a URL, beside
a no-op `open`. A two-step Graph (`lf -b research '<question>'`) reads each
step's answer from the transcript of the agent that finished it, and with the
whole order refused it waits (5, 15, 30, 60 minutes) where `lf -b <skill>`
stops: run it in the background. `agent_limit_recoveries` holds a row per move
to another provider for Graph steps only. Unset every `CLAUDE*` variable as
well as `LF_*` when the caller is itself a Claude session. When replacing a copied
`lf` that a server is running, `rm` it first: copying over the file in place
makes macOS SIGKILL the next exec (exit 137, no output).
Name the PTY driver anything but `pty.py`: it shadows the stdlib module.

# A provider's real spent limit

While a native login is out of quota the real refusal is free to use: a
disposable repository with `agents: [codex, claude:haiku]`, a one-step Graph
(`- step: say`, a skill named differently from the Graph) and the branch's
`lf -b graph <graph>` with only `LF_HOME` private prints
`codex: provider usage limit; continuing on claude:haiku` and Claude answers.
`agent_limit_recoveries.reason` holds the provider's typed reason. The
`lf task status` line "<step> went on under <agent> after: …" shows only while
the step runs, so give the skill something slow (a backgrounded `sleep 50`)
and poll. `agents: [codex]` or `-a codex` prints "Waiting until …" and waits;
killing that `lf -b graph` leaves its `lf --batch … skill <name>` child
waiting, so stop the child too. A Task launch also opens a Task conversation
on the first agent: find `codex` processes by their checkout cwd and stop them
with the home's `lf holder run`. For Codex's own failure lines, speak JSON-RPC
to `codex app-server` over stdio (`initialize`, `initialized`, `thread/start`,
`turn/start`) and read with blocking `readline`: `select` on a buffered pipe
misses `turn/completed`. An empty `CODEX_HOME` gives a 401
`httpConnectionFailed`, not `unauthorized`.
