Read when the change touches `lf repo tracker`, the `tracker:` config key, Git planning refs or Wave Project edits.

# Tracker

A fresh Git repository with no remote is the first-visit case: `lf repo
tracker` reads `Plan: this Machine.` and `lf task create`, `lf wave
edit-project` all run. Then `git init --bare` a sibling, `git remote add origin
<it>`, push `main`: `lf repo tracker` reads `Plan: Git origin, N unsent.` with
the records listed. `lf task sync <id>` (it needs an id) exchanges now, and
`git ls-remote origin` shows `refs/loopflow/plan/<machine id>`: one JSON file
per record, plus `repository`. Read one with
`git -C <bare> show "<ref>:tasks/<2 hex>/<task id>.json"`; in zsh write
`"${ref}:path"`, since `$ref:r` is a modifier.

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
same bare origin: its first `lf task list` fetches and shows the other
Machine's Tasks. Check `lf task list`, not only `lf task show <id>`: the list
reads by this clone's repository id, a lookup by Task id does not. Each clone
keeps its own repository id (`repository_plans`, `selected=1`) and holds the
others' as `selected=0` rows. On macOS name the homes without relying on
case: a home `T` beside a script `t` is the same path. A second clone under
the *same* `LF_HOME` is a different repository to lf and shows an empty plan;
its saves go out on the Machine's one ref.

Every save pushes its own ref at once, CLI or socket (about a second
against a local bare remote). Another Machine reads it on `lf task sync <id>`,
or on its server's minute (53 s measured). With a hub, the save is on the hub
when the command returns. To time it, poll `git -C <bare> for-each-ref refs/loopflow/plan`.

To make the remote fail, `git remote set-url origin /nonexistent.git`, save
and `lf task sync <id>`: `lf repo tracker` reads `unreachable since …` with
Git's reason, and `--json` fills `plan.git.error` and `failing_since`.
Restore the URL afterwards. A missing path fails at once; to see whether a
command keeps to its five seconds, make the remote hang instead: an
executable that runs `sleep 60`, set as `git config core.sshCommand <it>`,
with `remote set-url origin git@slow.example:x/y.git`. Time each command
(`python3 -I -c 'import time;print(time.time())'` before and after); with a
hanging remote or hub a save, a claim or a first read returns in 5 to 15 s
(a create reads the hub, then sends: two waits), `lf task sync` waits as
long as the carriers take. Unset `core.sshCommand` afterwards. For a first-wins claim race, point one home's
remote at the missing path while both run `lf task claim <id>`, restore it,
then `lf task sync <id>` on each; the losing home's steer is a row in
`steers` with `source` `ownership:<entry>`.

A hub needs no real host: put an `ssh` first on the laptop home's `PATH` that
runs its last argument with `bash -c` under the hub home's `HOME` and
`LF_HOME`, with the branch `lf` at that home's `~/.local/bin/lf`; then
`lf machine add mini` and `lf machine hub mini`. A flag file the stand-in
checks (exit 255) takes the hub down. For a sleeping mini, have a second
flag sleep 10 s before exiting 255 (SSH's `ConnectTimeout`); for a hub that
accepts and never answers, a third flag sleeps 120 s. `pkill -f` the
stand-in's own path afterwards, never a broader pattern. The hub must have fetched the laptop's
ref once before it knows the laptop's repository id.

For the returning-user case, build a home with the installed binary
(`whence -p lf`; `lf` is a shell function in zsh), which writes
`refs/loopflow/planning/shared/main`, then run the branch on that home: the
Tasks stay, and the first exchange publishes the whole plan to the new ref.

To check what a ref a newer build wrote does here, clone the bare origin,
`git fetch origin <ref> && git checkout FETCH_HEAD`, edit the JSON files and
push to `refs/loopflow/plan/home_<32 hex>`; skipped files and fields are
`WARN loopflow_execution::ops::plan_git` lines in `$LF_HOME/logs/lf.log`.

`lf server status` already prints compact JSON; it has no `--json` flag.

A Task the Machine hasn't saved goes through `ops/task.rs::fetch_from_tracker`:
Git refuses ("not found in the Git tracker"), Asana isn't fetched yet, and
Linear needs the team in the shared `.lf/config.yaml` (local config is
ignored there), an `origin`, then a Linear login. `lf task run <id> --explain`
reaches none of that; it says "is not registered".

`--explain` previews (`lf --explain task create|update|move …`) need no
registry and change nothing; `lf task delete --explain` has no planning
preview and prints context only. A bad `origin` URL does not make
`fetch_repository` return an error (the failure is stored on the destination),
so the post-creation `tracker fetch after creation failed` warning is not
reachable that way.
