# LOO-555 open questions (kickoff assumptions 2026-10-21; Jack to confirm)

- Cron visibility: resolved by Jack's 2026-10-21 steers: per-cron opt-in, one
  ref per Machine (`refs/loopflow/crons/<machine-id>`) holding only published
  crons. Assumed remote is `origin`. The bootstrap's release jobs stay private;
  `lf cron publish release-run` shows them.
- Repo crons not installed on Jack's laptop (intelligence `telemetry-daily`,
  product `wave/operate`) are dropped from config. The PR body lists their
  `lf cron add` lines. Whether mini-heart has them installed is unchecked.
- Person identity: resolved by Jack's 2026-10-21 steer: lowercase Git
  `user.email`, plus the "one of your Machines" rule. Assumed `git config
  --get user.email` in the command's directory, with no `LF_USER_EMAIL`
  forwarding: `lf --machine` claims rely on the remote's own email and the
  Machine rule.
- Wave Workflow definitions: assumed repository files only. The `wave_workflows`
  table is dropped, which also drops per-Wave same-name definitions. Intelligence's
  default `iterate` exists only in that table and references a `build` Flow
  that resolves nowhere. It is already unrunnable; after landing, select another
  Workflow there (for example `lf wave workflow set code intelligence`).
- Linear `branchName` may differ per viewer, which would give a second PR under
  Linear too. This was not verified, and this Task leaves Linear unchanged.
- LOO-550 landed on main as #1637 before this branch; the rebase rewrites its
  "Work with another person" caveats (takeover branch, Git comments, repo
  crons, identity) to this branch's behavior, keeping Jack's prose otherwise.
- Desktop's Workflow editor sheet is deleted with `--file`: Edit/Customize opens
  `.lf/workflows/<name>.yaml`, copying a builtin there first, like Flows.
- Ownership log items keep the `TaskOwnershipEntry` shape; `machine_label` is
  resolved for display when read, rather than adding a separate log line type.
- Public Machine name: implemented here (`lf machine public <name>|off`,
  `machines.public_name`) rather than deferred to 878ecf. A handoff entry to
  another of your Machines carries no label, since only the local Machine's
  public name is known.
- `lf refresh` under Git publishes pending local changes after acquiring, not
  acquire-only, so one command converges both people.
- Ownership rule refined from "key matches OR one of your Machines": a keyed
  entry is yours only when the key matches, so a teammate's claim on a Machine
  you registered stays theirs (`machine_commands`). The Machine rule covers
  keyless entries only. Your Machines need the same `user.email`.
- `two_people` installs Maya's cron through `ops::add_cron`, not `lf cron add`:
  the command refuses non-release builds (compile-time provenance). Publish,
  list and the public-name label run through the real CLI. The `lf cron add
  --publish` line itself is left to a release-build demo.
