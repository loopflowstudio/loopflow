Read when the change touches `lf repo tracker`, the `tracker:` config key, Git planning refs or Wave Project edits.

# Tracker

A fresh Git repository with no remote is the first-visit case: `lf repo
tracker` reads `Plan: this Machine.` and `lf task create`, `lf wave
create` both run. Then `git init --bare` a sibling, `git remote add origin
<it>`, push `main`: `lf repo tracker` reads `Plan: Git origin, N unsent.` with
the records listed. `lf task sync <id>` (it needs an id) exchanges now, and
`git ls-remote origin` shows `refs/loopflow/plan/<machine id>`: one JSON file
per record, plus `repository`. Read one with
`git -C <bare> show "<ref>:tasks/<2 hex>/<task id>.json"`; in zsh write
`"${ref}:path"`, since `$ref:r` is a modifier.

`lf config get repo.tracker` reads your `.lf/config.local.yaml`; pass `--shared`
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
and `lf task sync <id>`: `lf repo tracker` reads `failing since …` with
Git's reason, and `--json` fills `plan.git.error` and `failing_since`.
Restore the URL afterwards. A missing path fails at once; to see whether a
command keeps to its five seconds, make the remote hang instead: an
executable that runs `sleep 60`, set as `git config core.sshCommand <it>`,
with `remote set-url origin git@slow.example:x/y.git`. Time each command
(`python3 -I -c 'import time;print(time.time())'` before and after); with a
hanging remote or hub a save, a claim, a create or a first read returns in
about 5 s (a create's send reads the hub too: one wait), `lf task sync` waits as
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

`lf repo tracker --subscribe` needs `--json` and reads the store only (the
stream's store is read-only); run it in the background into a file and
`pkill -f` its exact command line. Off a TTY, `lf task show` prints JSON;
`lf task list | grep <id>` is the quick read of a title.

Two Machines making a Wave of one name is the import's hardest case:
`lf wave create design` on both homes (or `lf task create --wave design` on
one, which makes the Wave), a Task in each, then `lf task sync <id>` on both.
Do it offline (`git remote set-url origin /nonexistent.git` around each
create): online, the second create reads the plan first and joins the first
Wave. Each keeps its own `design` and its Tasks; everything else still
crosses. `lf task sync` exits 1 and `lf repo tracker` reads
`Plan: Git origin, 3 not taken.` with the rename under it (the other Machine
by id unless it was added with `lf machine add`). `lf wave rename design
<new>` on one home clears that home's line at once (a save reads too while
something is not taken); the other home clears on its next save, `lf task
sync <id>` or server minute. Both lists then hold all the Tasks and both
lines read `Plan: Git origin.` The skipped records are also
`WARN loopflow_storage::sqlite::plan_carriers` lines in each home's
`logs/lf.log`.

The hub home's server comes back after `pkill`: a command reaching it
through the stand-in `ssh` registers a launchd job for that home
(`com.loopflow.server.<hash>`, plist under `<home>/.lf/server.plist`). Find
it with `launchctl print gui/$(id -u)/<label> | grep path` and stop it with
`launchctl bootout gui/$(id -u)/<label>` when done. Replace the home's binary
(`rm`, then copy) before restarting it.

The hub carries the same case: take the hub down (the stand-in's flag) and
both remotes away, make the Wave on both, bring only the hub back and
`lf task sync <id>` on the laptop. Its line reads
`Hub mini, 3 unsent, 3 not taken.` with the reason; the rename on the laptop
clears it and the mini's list holds both. `lf task sync` still exits 1 on
Git's error while the remotes are away.

To exercise a Project ending through Git exchange, copy the Wave's
`projects/<id>.json` record and the `repository` file into a fresh Git
repository. Keep the Project ID and set `project_status` to `completed`
with a later `at` and the sending Machine's ID. Push the commit to
`refs/loopflow/plan/<that machine>` on the bare remote. After
`lf task sync <id>`, `lf wave status` shows the same Project as completed.
Fields this build doesn't know (`project_krs`,
`wave_project`) are `WARN loopflow_storage::work::edit: skipped an unknown
plan field` lines.

For a returning user on a stacked branch, the installed `lf` may be newer than
the branch base and its store refuses the branch ("migration … is unknown").
Build the base instead: `git worktree add --detach <dir> <base>` and
`CARGO_TARGET_DIR=<other dir> cargo build -p loopflow --bin lf` (about four
minutes), write the home with it, then open it with the branch.

For the minute, run the laptop's `lf server run` in the background (record
its PID), type nothing on the laptop and poll `lf task list` there after a
save on the mini: 40 to 60 s. The minute's exchange is silent; a split Wave
is one `skipped a plan record` WARN per exchange in `logs/lf.log`.

A Wave has exactly one Project, made with it (`ProjectId::for_wave`, the
Wave's UUID); a Task's Wave is its Project's. Read both in `loopflow.db`:
`projects(id, wave_id, project_name, status, external_project_id)` and
`tasks.project_id`; `tasks` has no `wave_id`. The plan's demo runs offline:
`lf wave create billing`, `lf wave create billing/refunds`, `lf task create
--wave billing/refunds 'x'`, `lf task move <id> --wave billing`, and
`lf wave status <wave>` prints `Project   <name> (local|linked) · <status>`.
For a returning user, write the home with the installed `lf` (it names a child
Wave's Project `billing/refunds`; the branch names it `refunds`), then open it
with the branch. Every recorded Task keeps its Wave. A second clone on another
home lists each Task under the same Wave after `lf task list`; a move on one
home reaches the other on `lf task sync <id>`.

A Wave's queued tracker actions are rows in `tracker_actions` (`kind`,
`phase`, `project_id`, `task_id`, `value_json`, `error`); with a Linear team
set and no login they stay `queued`, so a rename or removal shows its effect
there and in `lf repo tracker`. Renaming an unlinked Wave queues one `name`
action behind its Project's `create` (a second rename retires the first,
"superseded by a later save"); removing it retires both ("its Wave was
removed") and repoints a moved Task's queued `create` at its new Project.
The store has no WAL: copy `loopflow.db` and read the copy.

To see which Projects a newly connected tracker makes, remove a Wave first
(`lf wave remove`), then set `repo.tracker.linear.team` and an `origin`: `lf
repo tracker` lists one `create` per live Wave and every Task, none for the
removed Wave. With no Linear login every create and `lf wave plan connect`
stop at "No Linear credential found"; without `origin`, connect stops at the
origin error first.

A repository with a Linear or Asana tracker has no Git plan exchange: after
`git push`, `git -C <bare> for-each-ref refs/loopflow` is empty and a fresh
clone on a second home lists nothing. Cross-Machine checks there need a real
tracker login. With a team configured and an `origin`, every create waits on
"No Linear credential found"; without `origin`, on the origin error.

Without a Linear login no command links a Wave's Project. To see what a
linked one does (status line, its URL, a rename's `project_name` action),
write the link the tracker read would: set `projects.external_project_id`
and insert a `tracker_projects` row with the same `id`, `repo` (the
`/private/tmp/…` spelling `waves.repo` holds), `tracker` `linear` and a
`url`. `lf wave status <wave>` then prints `(linked)` and the URL under it.
This skips the tracker read itself, so it shows the branch's handling of a
link, not the import.

A Wave with no Project (`Project   none`, `"project": {"state": "ok",
"value": null}`) only arrives from another Machine: push a `waves/<uuid>.json`
record (`wave_name`, `wave_parent`, a `home_<32 hex>` machine) with the
`repository` file as above and `lf task sync <id>`. Its first Task makes the
Project.

To see what the branch does to this Machine's real data without touching it,
copy the store with SQLite's backup API (`sqlite3.connect(src).backup(dst)`)
into a private `LF_HOME`, and run the branch `lf` under `sandbox-exec -f` a
profile of `(allow default)`, `(deny network-outbound (remote ip))` and
`(deny file-write* (subpath …))` for the real checkout and `~/.lf`. Run it
from the real checkout: Waves belong to a repository by path, so a clone
lists none. Its first command applies the branch's migrations; the Linear
credential then fails to read, so every tracker action stays queued and
`lf repo tracker` lists what an upgrade would send. Delete the copy after
(about 1.4 GB with its backup).

`lf wave plan connect <new wave> <project>` with no Linear login fails at the
credential before it touches the Wave: no Wave, commit or queued action is
left. To see what a failed connect leaves, compare `lf wave list` and
`lf repo tracker` before and after, and read `tracker_actions` from a copy.

There is no Linear stand-in for a release `lf`: `LINEAR_BASE_URL` is a
constant, and reqwest verifies TLS with the macOS platform verifier, so a
local HTTPS proxy needs a keychain trust change (a prompt). A successful
connect or reclaim is reached only by execution's simulated-Linear tests
(`ops::linear::task_planning_tests::connect_*`).

What a removal leaves for the tracker shows offline: complete a Task in a
Wave, `lf wave remove` it, run any save (`lf task create 'x'`), then `lf repo
tracker`. A completed Task left in a removed, unlinked Wave's Project has no
open actions (retired "its Wave was removed"); with the link written by hand,
its `create` stays queued for the linked Linear Project. Do it once with the
team set before the Wave, once with it set after the removal.

A released Wave with two Projects (a chapter) is the slice-8 upgrade case.
The installed `lf` has no command that makes a second Project, and `lf wave
plan bind` refuses one ("Wave already has a different configured Project").
Make it arrive instead: before setting a tracker, push a `projects/<proj_…>.json`
record for the Wave (fields `project_wave`, `project_slug`, `project_name`,
`project_status`, stamped by another `home_<32 hex>` with a later `at`) to that
Machine's ref and `lf task sync <id>`; `lf wave status` lists it as
`historical`. Then set `repo.tracker.linear.team`, commit, push and save once:
the installed `lf` queues one Project `create` for the Wave's own Project and a
Task `create` per Task. Copy the home (`cp -R`), then run any branch command
on it. A Task record pushed the same way, with `task_project` naming the second
Project, is how a released writer's Task in that Project arrives at a branch
Machine.
