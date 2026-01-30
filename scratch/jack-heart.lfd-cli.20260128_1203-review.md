# Review: Unified step execution and Concerto continue button

## What was implemented

1. **Unified step execution module** (`execution.py`) — Consolidated step execution logic previously duplicated between `step.py` and `flow.py`. Both entry points now use `execute_step()` with an `ExecutionParams` dataclass. Interactive mode supports both `use_execvp=True` (single-step, replaces process) and `use_execvp=False` (flow, subprocess.run).

2. **Flow execution overview** — `lf flow` prints a header showing flow name, model, step count, and compact outline (e.g., `review → fork(reduce×3) → publish`) before execution.

3. **Improved step output** — Step headers use bold formatting and show model/direction/context config inline. Token profiles print before prompt finalization. Consistent structure between regular and interactive steps.

4. **Concerto Continue button** — Interactive sessions now show Cancel and Continue buttons in a footer bar. Continue sends EOF (Ctrl+D/ASCII 4) via `GhosttyManager.sendText()`, triggering graceful exit. Cancel destroys the session (SIGTERM). Keyboard shortcuts: Escape for Cancel, ⌘Return for Continue.

5. **Orphaned branch handling** — `worktrees.create()` detects and deletes orphaned local branches before creating a new worktree. `create_with_schema()` raises an explicit error when orphaned branch detected.

6. **`lfd reset` command** — Stops all running waves, deletes the database, reinitializes with latest schema. Requires `-f` for scripted use.

7. **Wave name resolution** — Commands `lfd status`, `lfd stop`, `lfd prs`, `lfd rm`, `lfd logs` accept wave names in addition to IDs.

8. **HTTP API cleanup** — Consolidated duplicate `StimulusUpdate` and `StimulusV1` models into single `StimulusRequest`.

## Key choices

| Decision | Why |
|----------|-----|
| `ExecutionParams` dataclass | Single source of truth for all execution config; avoids parameter sprawl across functions |
| Footer bar for Continue/Cancel | Header has status metadata; footer follows dialog conventions for action confirmation |
| EOF via Ghostty API | No new daemon API needed—existing exit code handling already advances flow |
| Silent orphan branch deletion | Matches `create()`'s existing worktree-reuse behavior |

**Fork step fallback** — When a fork thread doesn't specify a step, it now falls back to the fork-level step (e.g., `fork: {step: reduce, ...}`).

## How it fits together

```
step.py ─────┐
             ├──→ execution.py:execute_step() ──→ agent CLI
flow.py ─────┘

InteractiveSessionView
├── sessionHeader (status, wave name, step, badge)
├── terminalContent (GhosttyTerminalView)
└── sessionFooter (Cancel, Continue)
         └──→ GhosttyManager.sendText("\u{04}")
```

The `execution.py` module extracts shared logic: step-run creation, header printing, command building, and process management. `step.py` and `flow.py` now focus on context gathering and flow orchestration.

## Risks and bottlenecks

- **Silent branch deletion** — If a user manually created a branch with the same name as a worktree, `create()` deletes it without warning.

- **EOF mid-agent-response** — Clicking Continue while agent is mid-response sends SIGINT (exit 130), not graceful exit. Wave stays WAITING; user can reconnect.

- **`lfd reset` is destructive** — By design, but requires `-f` for scripted use.

## What's not included

- No changes to daemon execution output (only CLI `lf flow` output)
- No changes to flow parsing or DAG logic
- No test coverage for `_local_branch_exists`/`_delete_local_branch` (thin git wrappers)
- No agent completion detection in Concerto—users see terminal output directly
