# lf command reference

```bash
lf help --all
lf help pr land
lf process list --json
```

The command sections below are generated from the released `lf` when the website
builds; `lf help --all` prints the same tree locally. Canonical command paths are shown; unique shortcuts also resolve. See
[Get started](getting-started.md) for examples. This reference lists public commands.

## Selection and output

`--machine <label-or-id>` runs the command remotely; `--repo NAME_OR_PATH` overrides its saved repository.
`--forward-agent` requires `--machine`.
`--task` selects a Task checkout; `--wt` selects an existing worktree.
`--wave` adds context without moving directories and must match a Task's
owning Wave. `task run` places a Task's worktree and then runs like
`lf --task ISSUE graph GRAPH`; every Graph naming a Task is equally its work.

`-a` selects an explicit harness/model. Headless launches can follow configured
`agents` fallbacks when no explicit choice pins them; interactive launches
require an explicit retry or move. Claude and Codex use native authentication on
the execution Machine. Saved subscription accounts and old account preferences
do not select credentials.

JSON readers emit one document; a watched read such as
`lf process list --watch --json` emits NDJSON until stdin closes. Diagnostics go to stderr. Exit 0 means the requested
operation succeeded; 1 denotes an operational failure, 2 a syntax or lookup
failure, and 130 interruption. A successful auto-merge request is not a merge.

## Graph decisions and recovery

```bash
lf task run EXP-12 pursue
lf process show GRAPH_ID --json
lf task interrupt EXP-12
```

Each start is a new Graph: one lf process and the step processes it starts. Its ID
is the Graph process's. `--state` selects `current` (the Graph process has no recorded
exit), `completed` (it succeeded) or `stopped` (it exited before the last
step). A stopped Graph's Processes are its history; nothing resumes it. Inspect
them, then launch the work that remains.
