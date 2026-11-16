# Open choices (LOO-932)

- "One `lf` start between Start and a typeable terminal": the plan counts the
  Machine side (the held `lf session resume`, no exec into `lf -i`). Desktop's
  pane still runs a local `lf session resume ID --task X` relay to view the held
  terminal; removing it means feeding a Ghostty surface from a `session.attach`
  bridge in-app, which this plan does not do. The report counts both.
- A new Task's base is `origin/<default>` as of the last background fetch, not a
  fetch at Start (per the brief). A repository untouched for days starts days
  behind until its first sync. Taken as the simpler option.
- The ready checkout lives at `<parent>/<repo>.ready` beside Task worktrees,
  costing one checkout of disk per repository in use.
- Docs say `lf wt prune` leaves `<repo>.ready` alone, `--all` included, and
  that deleting it by hand is safe. The simpler reading of "check cleanup leaves
  it alone"; `--all` removing it would also be defensible.
- `lf -a X task create --run`: the agent override reaches the held
  conversation and its Graph only through inherited environment; creation no
  longer passes `--agent` to a `task run` argv. Not checked live.
