# W2-153 — Bound silent `lf ssh` cron-host probes

## Problem

`lf ssh mini-heart -- lf cron list` hung >60s with no output; the operator had
to interrupt it. `run_ssh` in `rust/loopflow/src/lf/commands/ssh.rs` spawns a
bare `ssh <host> bash -s` with no transport bounds. When the host is
unreachable, its key unknown, or password auth needed, `ssh` blocks — a
host-key / password prompt is read from the controlling **tty**, not our piped
stdin (which is already at EOF), so it waits forever. This silently defeats the
cron-host health probe: an unhealthy host stays quiet instead of surfacing.

## Fix (smallest safe)

Add standard noninteractive-SSH bounds in `run_ssh`, keeping the credential
preamble and secret handling untouched:

- `-o BatchMode=yes` — never prompt for password / host-key / passphrase; fail
  fast instead. This is the primary hang killer.
- `-o ConnectTimeout=<N>` — bound the connect handshake.
- `-o ServerAliveInterval=<N> -o ServerAliveCountMax=<M>` — bound a stalled
  *established* connection (dead network mid-session).

Classify the exit:
- `255` (ssh's reserved transport code) or death-by-signal → a bounded,
  sanitized `anyhow` error naming the **host** and the connection/transport
  phase. stderr is inherited so ssh's own reason is already visible.
- other nonzero → the remote command's own code; propagate via `process::exit`.

No credential value ever appears in the error. `-A` stays opt-in.

## Testable seams (pure fns)

- `ssh_args(host, forward_agent) -> Vec<String>` — assert it carries
  `BatchMode=yes`, `ConnectTimeout`, `ServerAliveInterval/CountMax`, host, and
  `bash -s`; `-A` only when forwarding.
- `classify_exit(code: Option<i32>) -> SshOutcome` — `Some(0)`→Success,
  `Some(255)`/`None`→ConnectionFailure, other→CommandFailure(code); the
  connection-failure message names the host.

## Out of scope

Host bootstrap, cron/release scheduling, Cadenza parity, release policy.
