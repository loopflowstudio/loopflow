# lf command reference

```bash
lf help --all
lf help pr land
lf session list --json
```

The command tree below is generated from the released `lf` binary when the
website builds; run `lf help --all` for the exact same tree locally. Canonical
command paths are shown, and unique shortcuts also resolve (for example
`lf land` → `lf pr land`). See [Get started](getting-started.md) for
walkthrough examples.

## Selection, output, and exit codes

- `--machine <label-or-id>` runs the command on a saved remote Machine;
  `--repo <name-or-path>` overrides its saved repository; `--forward-agent`
  opts into SSH agent forwarding.
- `--task <ID>` selects a Task checkout; `--wt <name>` selects a worktree;
  `--wave <name>` adds Wave context without changing directories.
- `-a <harness[:model]>` selects an explicit harness/model for the command.
  Headless launches without an explicit pin follow configured `agents`
  failover; interactive launches keep refusals visible.
- `--json` emits one JSON document; `--subscribe --json` streams NDJSON lines
  until stdin closes. Diagnostics print to stderr.
- Exit codes: `0` success, `1` operational failure, `2` syntax/lookup error,
  `8` checks still pending (`lf checks`), `130` interrupted. A successful
  auto-merge request is not a merge.

## Graph runs

```bash
lf task run EXP-12 pursue
lf graph show RUN_ID --json
lf task interrupt EXP-12
lf task pause EXP-12
lf task unpause EXP-12 --run
```

Each launch starts a new Graph run recording its own PID, steps, exit codes,
and errors (`--state current|completed|stopped`). A stopped run is history and
never resumes: inspect it, fix the cause, and run `lf task run` to continue
from the step it stopped on.
