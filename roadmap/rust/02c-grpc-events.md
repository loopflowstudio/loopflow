# Event Emission

The `EventHub` infrastructure is in place but events aren't emitted yet. Wire up `events.send()` calls:

- **Wave lifecycle**: `wave.started`, `wave.stopped`, `wave.waiting`
- **Agent lifecycle**: `agent.started`, `agent.ended`
- **Worktree changes**: `worktree.updated`, `worktree.pruned`

Emit from executor and store as state changes occur. Events are delivered to clients via WebSocket subscriptions.

## CollapsePRs

`CollapsePRs` needs an HTTP endpoint. The operation merges multiple wave PRs into one—used rarely.
