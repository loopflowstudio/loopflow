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
- `lf wt create ready` would plan the ready checkout's path. Not guarded;
  `WorktreeSegment` could reserve `ready` if it matters.
- The ready replacement is a background `git worktree add` from setup; if the
  Machine sleeps or it is killed mid-checkout, Git leaves a locked
  registration at the ready path and takes fall back to `worktree add` until
  someone removes it (`git worktree remove --force <repo>.ready`). No repair.

Answer on the "one `lf` start" choice (Jack Heart, 2026-11-16): "I guess this is fine, im not sure what the one lf start goal is about". Counting only the Machine side and keeping Desktop's pane relay stands.

Answer on `lf wt prune --all` (Jack Heart, 2026-11-16): "Yeah that seesm right".
Answer on `lf -a X task create --run` (Jack Heart, 2026-11-16): "ok, check it".
  Checked: the conversation ran X (its Session row's provider); the Graph did not. Fixed for the CLI. Desktop's served `task.create` carries no agent; adding one waits for a picker that sends it.
- Slice 4 removes about 0.58 s (debug, loaded Machine) of exec between setup and the `-i` launch; the prompt build after it costs 1.49 s. Order kept as planned (slice 4 next); prompt build is a candidate for its own Task.
- Phone Start records no Machine kind; every phone Machine is remote to the phone.
