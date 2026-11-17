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
- `lf -a X task create --run`: reading the code, the override does not reach
  the held conversation or its Graph. Steer (Jack Heart: "ok, check it"): a
  fixture with a non-default stand-in, then carry the choice in the request
  and the held argv.
- `lf wt create ready` would plan the ready checkout's path. Not guarded;
  `WorktreeSegment` could reserve `ready` if it matters.
- The ready replacement is a background `git worktree add` from setup; if the
  Machine sleeps or it is killed mid-checkout, Git leaves a locked
  registration at the ready path and takes fall back to `worktree add` until
  someone removes it (`git worktree remove --force <repo>.ready`). No repair.

Answer on the "one `lf` start" choice (Jack Heart, 2026-11-16): "I guess this is fine, im not sure what the one lf start goal is about". Counting only the Machine side and keeping Desktop's pane relay stands.

Answer on `lf wt prune --all` (Jack Heart, 2026-11-16): "Yeah that seesm right".
Answer on `lf -a X task create --run` (Jack Heart, 2026-11-16): "ok, check it".
  Reading of the code at 8dafb8c4b7: `-a` lands in `settings::invocation_values()` (in memory), not `LF_AGENT_OVERRIDE`. `hold` builds `lf session resume <id>` with no `--agent`, and `transport::start_held` copies only `std::env::vars_os()` plus the Wave ID. `start_in_background` gates on `config.agent()`, which ignores the override. So the choice looks lost for the held conversation and its Graph; a served `task.create` (Desktop) has no override in its environment at all. Needs a fixture that runs `lf -a <other> task create --run` and asserts the Session's provider, then the fix.
