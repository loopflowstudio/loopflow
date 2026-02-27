# Review: Remove completed status format item from wave backlog

## What was done

Removed the `@loopflow_status_format` customization item from `wave/tmux/05-polish.md`. The feature was already implemented and merged in commit `6c7473ee1` (PR #472).

## Key choices

- **Remove, not mark done.** Wave items are deleted once shipped, not crossed off. Keeps the backlog file focused on remaining work.

## How it fits together

`wave/tmux/05-polish.md` tracks follow-up polish items for the tmux plugin. One item (status format customization) is now shipped. The remaining item (interactive test coverage) stays.

## Risks and bottlenecks

None. This is a backlog cleanup.

## What's not included

- The `scratch/tmux-status-format.md` design doc from the implementation work remains in scratch/. It will be cleaned up on PR land.
