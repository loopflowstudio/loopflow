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

- Machine: `codex app-server daemon restart` with no thread loaded. The daemon
  now reports `jack@loopflow.studio`.
- Code, uncommitted and not yet compiled: `lf account codex use` asks the
  daemon for its login after activating (also when the file was already
  right) and restarts it on a mismatch. `daemon_login` / `restart_daemon` in
  `provider_auth/codex.rs`, called from `use_account`.

## Left to do

- Build is blocked machine-wide: `syspolicyd` (pid 494) is at 100% CPU and
  every newly compiled binary hangs in `_dyld_start`, including Cargo build
  scripts. Needs `sudo killall syspolicyd` from Jack.
- Then: `cargo test -p loopflow --lib provider_auth::codex`, clippy, and a
  live replay: `lf account codex use loopflow-eng`, then `use jack@…`; each
  should print the restart line and leave the daemon on the chosen login.
- Unverified: whether a stale daemon writes its old tokens back to
  `auth.json` on refresh. `auth.json` stayed on `jack@` for the hour observed.
- Automatic switches during an lf launch (`SwitchCause::Exhaustion`) leave the
  daemon alone, so a bare `codex` can lag after one. Restarting there would
  interrupt interactive work nobody asked to interrupt; undecided.
- Unrelated leak seen in passing: ~25 orphaned `codex app-server daemon
  pid-update-loop` processes under `/private/tmp/lf-title-*/home`, started
  Oct 8 13:19–15:19.
