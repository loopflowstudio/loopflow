Read when the change touches `lf machine login`, `logout`, `logins` or the launch login check.

# Real clients on a real Machine, no account touched

Drive the real `codex` up to the approval page; approving needs a person's
account, so a command approver (`approvers.codex.command`) records
`LF_APPROVE_URL`/`LF_APPROVE_CODE` and exits 1 (declined). Real Codex codes
look like `ZU80-PBMHR`. Put the stand-in `open` on `PATH` before the first
login, `--refused` included: without it the real browser opens the device page.

Claude here is plain `claude auth login` on lf's terminal: drive it with
`pty.fork()` and send `\x03`. Claude on another Machine is the forwarded
token: run the launch under `doppler run --` from this checkout (its project
holds `CLAUDE_CODE_OAUTH_TOKEN`), with `lf set --global forward.env
'[CLAUDE_CODE_OAUTH_TOKEN]'` in the private home. From a repository outside
this checkout, pass the checkout's Doppler scope explicitly: `doppler run
--project "$(doppler configure get project --plain)" --config …` (read both
in the checkout first). Outside `doppler run` the token is left out: Codex
launches run and Claude reads signed out (`… has no value here` in `logins`).
A real `lf --machine mini -b -a claude : '…'` answers in about 10 s.

`--refused` reads `agent_limit_recoveries`, which only a Graph step writes; a
bare `lf -b` refusal leaves no row, so `logins` shows `refused: null`.
mini's non-login shell has no `claude` on `PATH`; lf's own launch preamble
finds `~/.local/bin/claude`, plain `ssh mini-heart claude` does not.

Here: unset `LF_*`, private `LF_HOME`, and `CLAUDE_CONFIG_DIR`/`CODEX_HOME`
under it, so both read signed out and the person's logins stay untouched.
Put a stand-in `open` first on `PATH` that appends its argument to a file.
Claude Code 2.1.326 in a hyperlink-capable terminal (`TERM_PROGRAM=rex`) wraps
its URL in OSC 8, so check the exact bytes the approver got, with `repr`.

Another Machine (`mini-heart` on the tailnet): copy the branch `lf` to
`~/lf-qa-957/bin/lf` there, and put a Python `ssh` wrapper first on the local
`PATH` that prefixes the remote command with
`export LF_HOME=… CLAUDE_CONFIG_DIR=… CODEX_HOME=…` under `~/lf-qa-957` and
rewrites `export PATH="` to put that bin first, then execs `/usr/bin/ssh`.
`lf machine add mini-heart --label mini` through the wrapper, so the recorded
identity is the isolated home's. To check skew, a second wrapper keeps the
homes but not the bin (the installed released `lf` answers). To check an
unreachable Machine, swap in an `ssh` that prints a timeout and exits 255.
Plain `ssh` from a shell that sourced the wrapper's `PATH` also reads the
isolated homes: check the real logins from a clean shell.

After every scenario, `ps -axo pid,etime,command | grep '[c]odex login'`
on the target: closing a PTY-less SSH exchange signals nothing remote, so a
login that outlives lf shows up only there. Name a `pty.fork()` driver anything but `pty.py`. In zsh, loop over commands
as arrays (`run(){ lf "$@"; }`): `lf $c` with a string passes one argument.

The launch check: in a fresh Git repo with a local identity, `lf -b -a codex
: 'say hi'` and `lf --machine mini -b -a codex : 'say hi'` stop before the
agent with the `lf machine login` command. Bare `lf -b` never walks `agents`;
only a Graph step moves to the next provider.

Services here: Asana takes its OAuth client from Doppler, so run
`lf machine login asana` from this checkout (its Doppler project holds
`ASANA_CLIENT_*`); from `/tmp` it fails with "asana CLI not
found". A command approver receives the real authorize URL and declining frees
port 19222. Doppler reads the person's real `doppler` CLI login and finishes
with no page when it is signed in. To check that a provider never reaches
another Machine, log every `ssh` argv from the wrapper and count lines.
