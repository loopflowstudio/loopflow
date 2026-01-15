# Deferred Backend Work

## Agents

1. **Looping trigger** - `TriggerKind.LOOP` should run indefinitely, generating a diff per iteration. Current backend has the enum but no execution logic.

2. **Pipeline storage for agents** - Agents create worktrees, so they can't store pipelines in the worktree (doesn't exist yet) or in main (shouldn't edit). Options:
   - Store in `~/.lf/pipelines/` (global)
   - Store inline in agent definition
   - Reference repo pipelines by name (current approach)

3. **Auto-close worktrees** - After an agent's PR merges, clean up the worktree automatically. Needs: merge detection, worktree removal, branch cleanup.

4. **Goal file watching** - Re-trigger agent when its goal doc changes. Needs: fsevents/inotify watcher in the daemon.

## Voices

5. **Voice creation flow** - UI to create new voices from Maestro (deferred, currently voices must exist on disk).

## Pipelines

6. **Pipeline visualization** - Show pipeline DAG in UI (either as third view or inline in agents/voices).
