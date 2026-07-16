# Reap orphaned provider processes and classify their children accurately (W2-259)

## Problem

When an `lf wave` (or any `lf` body driving an OpenCode harness) crashes or
even stops cleanly, the `opencode serve` process and the children it spawns
(MCP servers, model proxies, node/npm-shim descendants) are left behind.

Two concrete gaps:

1. **OpenCode harness doesn't isolate its process group.** `opencode serve` is
   spawned in the runner's inherited process group. `stop()` does
   `child.start_kill()` (SIGKILL on the direct child only) + `wait`, so any
   descendant survives as an orphan. `process_group_id()` returns the trait
   default `None`, so the Session receipt never records a provider group and
   `reap_child_process` (recovery) has no group to kill. Codex solved exactly
   this (`command.process_group(0)` + `kill_process_group(pid)` on stop + an
   interrupt-cleanup hook); OpenCode was never brought up to it.

2. **The startup reaper kills only the direct PID, and isn't even called.**
   `reap_orphaned_opencode_servers` walks a registry of `opencode_pid ->
   owner_loopflow_pid` and, for dead owners, `kill -TERM <opencode_pid>` — the
   single PID, not its children. It also short-circuits when the leader PID no
   longer matches `opencode ... serve`, which means: if the leader died but its
   children survive (still in the group, re-parented to pid 1), the entry is
   pruned and the children stay up. And the public entry point is never invoked
   anywhere — the doc says "the per-wave runtime can call it at startup," but no
   caller exists.

## Design

### Harness: own process group, kill the tree (`harness/opencode.rs`)

Mirror codex:

- `command.process_group(0)` on spawn (unix) → the child becomes group leader,
  pgid == child pid. All descendants stay in that group unless they explicitly
  `setpgid` (these vendor processes don't).
- Track the group in an `AtomicU32` (`child_group`); implement
  `process_group_id()` returning it. The runner already feeds
  `harness.process_group_id()` into `observe_provider`, so the Session receipt
  records the provider group and `reap_child_process` will kill it on recovery.
- `stop()`: after the HTTP abort/delete, `kill_process_group(pid)` then
  `start_kill` + `wait` (same shape as codex). Clear `child_group`.
- Register an interrupt-cleanup hook once (SIGINT/SIGTERM/SIGHUP skip Rust
  destructors, so `kill_on_drop` never fires on the signal path — the hook is
  what keeps `tmux kill-session` from orphaning the group). `kill_on_drop(true)`
  as belt-and-suspenders for a non-signal drop without `stop()`.
- `unregister_opencode_server(pid)` stays (registry hygiene).

### Reaper: kill the group, classify the leader (`harness/opencode_runtime.rs`)

The registry's `opencode_pid` is the group leader (pgid == pid under
`process_group(0)`), so `kill(-(opencode_pid))` targets the whole tree. That is
"classify children accurately": the group contains exactly the server + its
descendants, never unrelated processes.

Replace the `process_matches_opencode` + `terminate_pid` closures with a
leader-classification + group-reap model. Per entry whose owner is dead:

- `Opencode` (leader alive, command matches `opencode ... serve`): the common
  orphan shape — reap the group.
- `Dead` (leader gone) + group still has members: the leader died but children
  survive — reap the group to take the children.
- `Dead` + group empty: whole tree gone — prune the entry.
- `Other` (leader PID reused by an unrelated process): don't kill someone else's
  group — prune and leave it.

Reap = bounded SIGTERM the group → wait → SIGKILL. Guard against reaping the
*current* process group (the reaper runs inside a live `lf`). Use `libc::kill`
directly (unambiguous negative-pid = group semantics); the existing
`ps`-based command match stays as the pid-reuse guard.

### Wire-up: call the reaper at resident boot (`wave/resident.rs`)

`reap_orphaned_opencode_servers()` runs once at the top of `resident::run`,
before the loop starts, logging the report. A resident is the long-lived body
that spawns OpenCode servers; sweeping orphans from a previously crashed
resident at boot is the documented intent. The sweep is machine-global over the
LF_HOME registry.

## Tests

- Harness: a `cfg(unix)` `tokio::test` mirroring codex's
  `kill_process_group_reaches_the_grandchild` — the direct child backgrounds a
  grandchild in the same group and exits; `stop()`'s group kill takes the
  grandchild down before a flag file appears.
- Reaper: `_at_path` unit tests covering each `LeaderState` arm — `Opencode`
  reaps the group, `Dead`+group-alive reaps surviving children, `Dead`+group-dead
  prunes cleanly, `Other` is left alone, owner-alive is retained. Idempotency
  preserved.

## Out of scope

- Deduplicating `kill_process_group` across codex / provider_auth / opencode
  (pre-existing; trivial follow-up).
- Calling the reaper from one-shot Task/Project runners (resident boot is the
  documented site; lazy sweep-on-spawn is a tunable for later if orphans from
  crashed one-shots become a measured problem).
