# Automatic disk recovery

## Failure

Loopflow created enough sibling worktrees for replicated Rust and Xcode build
artifacts to consume the disk. `lf wt prune` could reclaim them, but only when a
human ran a full repository scan. The existing `autoprune` config was never read.
`lf land` promised post-merge self-pruning without a daemon or webhook path that
could fulfill it.

Interrupted prompt-log writes also left more than fifteen thousand `.tmp*`
directories under `~/.lf/logs`.

## Design

- Move prune selection/removal into the worktree engine so the CLI and daemon
  share one implementation.
- Enable lossless autoprune by default. `lfd` sweeps immediately and on a bounded
  interval, removing clean merged, remotely deleted, and terminal Task
  worktrees. Dirty worktrees remain visible for recovery.
- Accept signed GitHub `pull_request` and `delete` webhooks for targeted cleanup.
  When a callback URL is configured, register those subscriptions on daemon
  startup without putting the signing secret in argv or service files.
- Remove only direct `~/.lf/logs/.tmp*` directories older than 24 hours. Durable
  trace artifacts remain ledger-owned.
- Keep line-table debugging but disable incremental compilation for development
  and test profiles. Agent worktrees typically build once; their incremental
  state alone consumed 4–6 GiB per checkout.

The clean `loopflow` library-test build is 1.0 GiB with this profile, down from
the 9–17 GiB observed in existing agent worktrees; `target/debug/incremental`
is empty.

## Done when

- A merged/deleted GitHub branch removes its clean local worktree without a full
  manual scan.
- Periodic recovery catches missed events and cleans stale Git registrations.
- Dirty worktrees are never automatically deleted.
- Nonterminal Task Sessions and live process workspaces survive every prune
  path, even when GitHub says their branch landed or disappeared.
- Abandoned prompt-log staging directories age out automatically.
- `lf wt prune` is the explicit sharp edge: it removes every worktree without
  live process or nonterminal Task ownership, including dirty/unmerged work.
