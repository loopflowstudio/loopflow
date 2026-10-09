# Codex daemon keeps the old login after `lf account codex use`

## Symptom

Jack ran `lf account codex use jack@loopflow.studio`, which printed "already
signed in", yet a bare `codex` ran as `loopflow-eng@loopflow.studio`.

## Observed (2026-10-09)

- `~/.codex/auth.json` held `jack@loopflow.studio`.
- Codex's managed background app-server (pid 25446, started Oct 8 12:22)
  answered `account/read` on `~/.codex/app-server-control/app-server-control.sock`
  with `loopflow-eng@loopflow.studio`. It reads `auth.json` only at start.
- A bare `codex resume` (pid 15208) held a connection to that socket. Codex
  terminals launched with `-c` overrides (lf's and cmux's) did not, and the
  daemon listed none of their threads.
- The app-server protocol has no call that reloads a login from disk
  (`generate-json-schema`), so a restart is the only way to make it adopt one.

## Repair

- Machine: `codex app-server daemon restart`. The daemon reports
  `jack@loopflow.studio`, matching `auth.json`.
- Code: `lf account codex use` asks the daemon for its login after activating
  (also when the file was already right) and restarts it on a mismatch.
  `daemon_login` / `restart_daemon` in `provider_auth/codex.rs`, called from
  `use_account`.

- Code: lf's Codex terminal launches pass `--no-daemon`. They already stayed
  off the daemon, apparently because Codex embeds its server when given
  configuration overrides (inferred from the binary's "remove configuration
  overrides or use --remote" message, not traced in Codex source). The flag
  states it. Checked against codex-cli 0.161.0, which accepts it ahead of
  `resume`, `fork` and `exec`; an older Codex without the flag would reject
  the launch.

## Verification (2026-10-09)

- `cargo test -p loopflow --lib provider_auth::codex`: 4 passed.
  `cargo clippy -p loopflow --all-targets -- -D warnings`: clean.
- Counterexample, installed 0.13.10: `use loopflow-eng` rewrote `auth.json`
  and left the daemon on `jack@` (same pid).
- Fixed build, run against a disposable `LF_HOME` holding copies of the Codex
  account rows (a dev build otherwise forwards to the installed `lf`):
  `use loopflow-eng` and `use jack@…` each printed the restart line and the
  daemon answered with the chosen login under a new pid; a repeat `use jack@…`
  left the pid alone.
- That replay restarted the daemon while thread `01a11dc6` (cwd
  `loopflow.flow-skills`) was mid-turn. It reloaded and is idle; its turn was cut.

## Open

- Jack chose a bounded wait: `use` installs the login first, releases the
  login lock, then gives turns running through the daemon up to five minutes
  before restarting it. Busy detection is unit-tested; the wait itself has not
  run live, because the only running turn available was Jack's own session.
- Automatic switches during an lf launch (`SwitchCause::Exhaustion`) leave the
  daemon alone, so a bare `codex` can lag after one.
- Unverified: whether a stale daemon writes its old tokens back to
  `auth.json` on refresh. Not observed over about an hour.
- Unrelated: ~25 orphaned `codex app-server daemon pid-update-loop` processes
  under `/private/tmp/lf-title-*/home`, started Oct 8 13:19–15:19.
- Unrelated: `syspolicyd` wedged at 100% CPU and hung every new binary at
  launch until Jack ran `sudo killall syspolicyd`.
