Read when the change touches `lf machine login`, `logout`, `logins` or the launch login check.

# Real clients on a real Machine, no account touched

Drive the real `claude` and `codex` up to the approval page; approving needs a
person's account, so a command approver (`approvers.<provider>.command`)
records `LF_APPROVE_URL`/`LF_APPROVE_CODE` and exits 1 (declined) or prints a
code. Real Claude answers a malformed code with a stderr complaint and waits
for another paste; a well-formed wrong one exits 1. Real Codex codes look like
`O1DJ-RMBI6`.

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

After every scenario, `ps -axo pid,etime,command | grep '[c]laude auth login'`
on the target: closing a PTY-less SSH exchange signals nothing remote, so a
login that outlives lf shows up only there. Drive a person at a terminal with
`pty.fork()` (paste a code with `\r`, Ctrl-C as `\x03`); name the driver
anything but `pty.py`.

The launch check: in a fresh Git repo with a local identity, `lf -b -a codex
: 'say hi'` and `lf --machine mini -b -a codex : 'say hi'` stop before the
agent with the `lf machine login` command. Bare `lf -b` never walks `agents`;
only a Graph step moves to the next provider.
