---
extends: builtin
---
## User interface changes

Under **What changes**, list each thing people use that the diff adds,
removes or renames, before **Model changes**:

- Commands: `lf help --all` prints the public command tree. Compare it before
  and after: "Adds `lf skill customize`", "Removes `lf transport start`".
  Mention changed arguments only when they change what someone types.
- Views: each Desktop or phone screen, pane, tab, menu item or setting
  (`swift/`), and what `lf open` shows in Rex, Herdr or cmux.

For each addition, name the Task or design it delivers, or say none was
recorded. Keep this list even when the change looks minor; write "None" when
nothing moved.

## Model changes

Under **What changes**, give the stored model before and after: run
`uv run python scripts/model_size.py` on the branch and on the PR base (a
detached `git worktree add` of the base, removed afterwards) and write each
count as `before → after`: tables, columns, storage `pub fn`s and SQL call sites
outside `loopflow-storage`. Name each table or column added or dropped, and
each `-- leaving:` mark with its Task.
