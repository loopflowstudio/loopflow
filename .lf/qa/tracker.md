Read when the change touches `lf repo tracker`, the `tracker:` config key, Git planning refs or Wave Project edits.

# Tracker

A fresh Git repository with no remote is the first-visit case: the default
tracker is Git on `origin`, so `lf repo tracker`, `lf task create` and
`lf wave edit-project` all run without one. Then `git init --bare` a sibling,
`git remote add origin <it>`, push `main`, and save once: `git ls-remote origin`
shows `refs/loopflow/planning/shared/main`.

`lf config get repo.tracker` reads your `lf.config.local.yaml`; pass `--shared`
for the committed `.lf/config.yaml`. A dotted set
(`lf config set --shared repo.tracker.linear.team T`) writes the released
`planning: {provider, …}` shape. Quote numeric Asana team IDs (`"'1209'"`):
YAML reads them as integers.

Linear status needs no Linear login: an `origin`-less repo with a Linear team
queues every create and `lf repo tracker` lists them as open, with the reason.

The served route is `repo.tracker` with `{"directory": …}`; send it after
`hello` on the private server's socket (see `server.md`).

An installed `lf` older than #1957 writes `task_changes`, not
`tracker_actions`, so its Tasks never show as open actions to a branch build.
That skew is #1957's, not a vocabulary rename's; compare against an `lf`
built from main for mirror-table checks.

The `sqlite3` CLI here refuses `-readonly` on a WAL store (error 14); read with
Python: `sqlite3.connect("file:…/loopflow.db?mode=ro", uri=True)`.

A second Machine is a second `HOME`/`LF_HOME` pair on a fresh clone of the
same bare origin: its first save fetches and imports the shared ref, and
`lf task list` shows both Machines' Tasks. A second clone under the *same*
`LF_HOME` is a different repository to lf; its import fails with "planning
record belongs to another repository", which `lf repo tracker` and the
`fetch_error` field only report as "Git fetch failed; retained import
unchanged". Nothing logs the cause; to see it, print the error in
`ops/planning_sync.rs::exchange_destination` in a throwaway build.

To make a fetch fail, `git remote set-url origin /nonexistent.git` and save;
the destination's `fetch_error` and `publication_error` both fill in. Restore
the URL afterwards.

`lf server status` already prints compact JSON; it has no `--json` flag.

A Task the Machine hasn't saved goes through `ops/task.rs::fetch_from_tracker`:
Git refuses ("not found in the Git tracker"), Asana isn't fetched yet, and
Linear needs the team in the shared `.lf/config.yaml` (local config is
ignored there), an `origin`, then a Linear login. `lf task run <id> --explain`
reaches none of that; it says "is not registered".
