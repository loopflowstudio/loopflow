# LOO-627 open questions (kickoff, 2026-10-27)

Jack Heart left these open in the Task. The plan assumes the reversible choice below
for each; the design (`scratch/loo-627-new-task.md`) depends on them.

- **One command or two?** Assumed one: `lf task create "<request>" --run [--graph G]`,
  which creates the Task, runs it, and opens its conversation. `create` and `run`
  still work on their own. Renaming to `lf new` or `lf task new` later is cheap.
- **Where the button lives.** Assumed:
  - ⌘N and File ▸ New Task, through the Desktop action registry (`lf desktop new-task`)
  - an always-visible New Task row at the top of the sidebar Task list
  - not in the Wave view (LOO-594)
- **What the first screen shows.** Assumed: a sheet with
  - the request field
  - a Wave picker that defaults to the sidebar scope
  - a Workflow picker that defaults to the resolved default and names its source,
    with a line saying where it stops for you
- **When `--run` opens the conversation in the terminal.** Only when stdin and stdout
  are a terminal and `--json` is not given. Otherwise it admits the conversation and
  prints `lf resume --task <ID>`, so agents calling it are never dropped into an
  interactive provider.

## Resolved at explore (2026-10-28)

- Jack Heart chose prototype A, the sheet (`scratch/new-task-prototypes/index.html`), over talk-first and the one-line palette.
- Jack Heart chose the top of the Task list for the New Task row (`scratch/new-task-prototypes/placements.html`), over the title bar and a per-Wave ＋.

## Implementation assumptions (2026-10-28)

- **A run that fails after create, in Desktop.** The sheet stays open with the error
  and the request; the filed Task appears in the sidebar with its Start control.
  Pressing Create again files a second Task. Simpler than a retry path; revisit if
  the demo shows it happens.
- **Stops line for a graph with no interactive step** reads "runs to its end without
  stopping", not "runs to a merged PR": a custom graph need not open a PR.
- **LOOPFLOW.md's agent handoff** keeps `create --title` then `task run`; agents keep
  that path, so only the README moved to `create "…" --run`.
