---
extends: builtin
---
## Command changes

`lf help --all` prints the public command tree. When the diff adds, removes or
renames a Clap command, compare that tree before and after the change and list
each added or removed command under **What changes**: "Adds `lf skill customize`",
"Removes `lf transport start`". Mention changed arguments only when they change
what someone types. For an added command, name the Task or design it delivers,
or say none was recorded. Keep this list even when the change looks minor.

## Model changes

Under **What changes**, give the stored model before and after: run
`uv run python scripts/model_size.py` on the branch and on the PR base (a
detached `git worktree add` of the base, removed afterwards) and write each
count as `before → after`: tables, columns, storage `pub fn`s and SQL call sites
outside `loopflow-storage`. Name each table or column added or dropped, and
each `-- leaving:` mark with its Task.
