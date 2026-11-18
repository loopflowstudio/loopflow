Read when the change touches Git planning exchange, the planning journal or when a foreground Session fetches.

# Git planning on private Machines

One bare repository stands in for the planning remote; each Machine is its own
`LF_HOME`. Give every checkout a remote named in config, not `origin`:

```sh
git init -q --bare plans.git
git init -q G; cd G; mkdir .lf; git remote add plans ../plans.git
printf 'planning:\n  provider: git\n  remote: plans\n  shared: qa\n' > .lf/config.yaml
git add -A; git commit -qm config; git push -q plans main:G-main
git clone -q -o plans ../plans.git ../G2 -b G-main   # second Machine's checkout
```

A Linear repository needs no Linear account for journal checks:
`provider: linear` + `team: offline-fixture` saves locally and queues
`tracker_actions` (the drain fails on the missing `origin`, which is fine).

- A second Machine learns a Task through `lf task sync <full id>` (it acquires)
  or a foreground tick. `lf task show <id>` of a Task it never imported says
  "is not registered"; it does not acquire.
- Count network exchanges with a `git` shim first on `PATH` that appends
  `$(date +%s) $PWD $*` to a log and execs `/usr/bin/git`. An acquisition is
  `ls-remote` then `fetch`; a publication adds `push`. Match the subcommand
  word, not `/push/`: `remote get-url --push` runs on every exchange.
- A foreground Session without a provider login: a stand-in `claude` on `PATH`
  that sleeps, then `$LF_BIN -b default 'x' -a claude < /dev/null`. The
  planning thread runs while it sleeps; the run then fails "exited without a
  final result", which is expected.
- Block a push with `chmod -R a-w plans.git`; restore with `chmod -R u+w`.
- Journal rows: `planning_peer_changes` joined through `tasks.repository_id` →
  `repository_plans.repo`; the recorded tracker is
  `repository_plans.planning_tracker`.
- Migration on real data: `sqlite3 "file:$HOME/.lf/loopflow.db?mode=ro" ".backup <copy>"`
  (1 GB, about 10 s), point `LF_HOME` at the copy's directory and run any
  store-opening command (`lf machine status`) from an empty directory.
