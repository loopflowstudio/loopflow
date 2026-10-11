# lf command reference

```bash
lf help --all
lf help pr land
lf monitor --json
```

Generated from the compiled Clap tree. Canonical command paths are shown; unique shortcuts also resolve. See
[the workflow guide](lf.md) for examples. Hidden commands
are internal process boundaries and are marked below.

## Selection and output

`--machine <label-or-id>` runs the command in the saved remote repository.
`--forward-agent` requires `--machine`.
`--task` selects a Task checkout; `--wt` selects an existing worktree.
`--wave` adds context without moving directories and must match a Task's
owning Wave. `task run` places a Task's worktree and then runs like
`lf --task ISSUE run FLOW`; every Flow naming a Task is equally its work.

A preference (`--account`) permits fallback. A restriction (`--only-account`)
limits this launch and its children. A Flow retains its provider selections;
children check their destination's access before starting a provider.
Account observations distinguish unavailable, expired and measured capacity.

JSON readers emit one document; `monitor active --watch --json` emits NDJSON
until stdin closes. Diagnostics go to stderr. Exit 0 means the requested
operation succeeded; 1 denotes an operational failure, 2 a syntax or lookup
failure, and 130 interruption. A successful auto-merge request is not a merge.

## Flow decisions and recovery

```bash
lf task run EXP-12 pursue
lf flow show FLOW_ID --processes --json
lf task interrupt EXP-12
```

Each start is a new Flow: one lf process and the step processes it starts. Its ID
is the Flow process's. `--state` selects `current` (the Flow process has no recorded
exit), `completed` (it succeeded) or `stopped` (it exited before the last
step). A stopped Flow's Processes are its history; nothing resumes it. Inspect
them, then launch the work that remains.

## lf

Open Loopflow or run its CLI

| Argument | What it does |
|---|---|
| `--docs` | Docs paths, globs, or directories to include in context |
| `--clipboard / -c` | Include clipboard content in prompt Default: false. |
