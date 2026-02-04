# gRPC Event Emission

The `EventHub` infrastructure is in place but events aren't emitted yet. Wire up `events.send()` calls:

- **Wave lifecycle**: `wave.started`, `wave.stopped`, `wave.waiting`
- **Agent lifecycle**: `agent.started`, `agent.ended`
- **Worktree changes**: `worktree.updated`, `worktree.pruned`

Emit from executor and store as state changes occur.

## CollapsePRs RPC

`CollapsePRs` is not in the proto. Options:

1. Add `CollapsePRs` RPC to `control.proto`
2. Keep HTTP fallback in Swift for this single operation

The operation merges multiple wave PRs into one - used rarely, so HTTP fallback may be acceptable.
