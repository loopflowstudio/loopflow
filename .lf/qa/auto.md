Read when the change touches `lf task auto on`, `lf task auto`, the auto setting or a Task run's landing step.

# Auto on a private Machine

Set up a [private Machine and repository](served-launches.md) with the branch
`lf`. For PR cases, use the [GitHub stand-in](stacked-tasks.md) and create
disposable Tasks with merged, open and draft PRs and a Task with no PR. Mark
one PR merged in the stand-in's state and check that `land` reports it already
merged. Start the branch's `lf server run` before launching Tasks.

If the private Machine registers a launchd job, use the label in
`$LF_HOME/server.plist` to boot it out at cleanup. End the holders and providers
belonging to that private home as described in [resume](resume.md).

Graph: `.lf/graphs/qaship.yaml` with `- implement`, `- interactive:
ship-review`, `- cmd: land`, `- interactive: verify`; commit, push, then
`git merge origin/main` in each Task checkout (the graph is read from the
checkout). For a run with no interactive step ahead, `- implement` then
`- cmd: land`, on a fresh `lf task create '<words>' --json`.
`lf task auto on <id> --graph qaship` takes it up; the served `task.auto.on`
takes no graph, so for it set `graph: qaship` in `.lf/config.yaml` (committed,
pushed, merged into the checkouts) so a never-run Task takes it up.

`auto: on` in config: `lf config set auto on` writes your own value
(`.lf/config.local.yaml`), or commit it in `.lf/config.yaml` or
`.lf/wave/<wave>/config.yaml` (push, merge into the checkouts). Read it back with
`lf config settings`, `lf --wave <wave> config settings` and the `auto` line of
`lf wave status <wave>`. A bare `lf task run <id>` on a never-run Task then
prints the `merges:` line and status reads `auto: on by personal config`,
`by repository config` or `by wave config`. Create a Task inside the disposable
Wave and another outside it to check the scope of each setting. `on` is the
only value: `off`, `true` and `null` are refused. A released config's
`graph: auto` is checked by writing the file (`lf config settings` reads
`graph code`, `auto on`); `lf config set graph auto` refuses it. `lf -b task run
<id> <graph>` stands in at every step and leaves `auto` null in
`lf task status <id> --json`. For the released `lf` against the new key, a
fresh repository and `LF_HOME` with `auto: on` committed: 0.16.0 warns
"skipping auto: unknown setting" on each read and carries on.

Headless steps: a stand-in `claude` first on the private `PATH` that, for `-p`
with no terminal, reads one line, waits while a marker file exists, then
prints `{"type":"result","subtype":"success","result":"done",…}`; otherwise
it reads and echoes input from the PTY. Touch the marker to keep `implement`
running while you use `lf task auto on` or `lf task auto off`. Remove the marker
to advance the run.

Read the setting with `lf task status <id> | grep -E 'auto:|Graph q'` and
`lf task status <id> --json` (top-level `auto`; `lf task show <id> --json`
carries it as `execution.auto`). Served: hello from
`rust/loopflow-execution/tests/fixtures/dto/server/hello.json`, then
`{"type":"request","id":…,"operation":{"kind":"task.auto.on","directory":…,
"issue":…},"select":null,"sender":null,"request_id":"<uuid>"}`; `task.auto.on`
takes nothing else (a never-run Task takes up the repository's graph), and
`task.auto.off` takes `"reason":null` beside them and answers `{issue, was_on}`.
Check `lf task auto on` on a Task waiting at `ship-review` (it stands in
there) and on a never-run Task (one Graph run); `lf ship` names no command ("skill or graph not found: ship").
With `--json` the run is the only thing on stdout; "standing in at …" goes to
stderr. Two at the same moment on a never-run Task leave one run:
both print it and exit 0, and `lf task status` shows one Graph run. The losing
paths are rare, so repeat it: a dozen fresh `lf task create '<words>' --json`
Tasks (`.task.identifier`), three `lf task auto on <id> &` each with the stand-in
holding `implement`, then `wait`; every exit is 0, every output names the same
Graph run, and `graph_processes` holds one row for the Task. Two bare
`lf task run` at once leave one run and one "is running graph …" refusal.

To check a blocked decision after a successful step, give the Graph a loop with
`cmd: help commit` followed by an interactive decision skill. Ask the skill to
check for an ignored artifact in the Task checkout and block with a reason if
it is missing. With a real provider, `lf task run <id>` waits at the interactive
step. `lf -b task auto on <id>` then exits 3, and `lf task status <id>` shows the
reason. Put the artifact in the checkout and repeat `lf -b task auto on <id>`.
`lf graph show <run-id>` lists the decision alone in both auto runs.

Both `--reason`s are Task comments (`lf task show <id> --json` at
`comments.comments`), and a person's comment is a steer: every later step's
Session lists it in `lf session steers <session> --json`, the off reason too.
The stand-in runs no hooks, so they stay `pending` there; a real Claude claims
them in its start hook. They are not in the step's stdin or `.lf/prompts`.

With auto off at a landing step, `lf task status` reads `latest Graph: Waiting
at land: auto is off (Demo). \`lf task auto on <id>\` lands it.` and `action: no_action`;
`execution.state` is `waiting`. The run's own log (`$LF_HOME/logs/graph-<run>.log`) ends in that sentence
alone; an interactive stop ends in "Stopped at <step>; the Task waits there for
its conversation." What a Task conversation is handed: `lf --task <id> --context
<skill>` prints its input (the Task brief, no Graph position), and each held
conversation's `--append-system-prompt-file` is in `ps` under its `lf holder run`.
An unknown ID answers `no Task exists for …`
(served: `not_found`) for `auto off`; `task.auto.on` still answers `outcome_unknown`.

Before a first run: `lf task auto off <id>` on a fresh `lf task create`, then
`lf config set auto on` and a bare `lf task run <id> <graph>` stops at the
interactive step with no `merges:` line. `lf task complete` then `lf task
reopen` clears the mark (`auto` null in `--json`; the row is in `task_auto`).
Across Machines (the two homes of `.lf/qa/machine-moves.md`): the mark arrives
with the Task, set or not, and replaces what the receiver held; the laptop
reads it back as `State: unstarted · auto: off by Demo, …` from mini's server.

Selection state `auto` is a Dashboard expression (`lf desktop dashboard query
'Auto' 'state:auto'` prints JSON), not a `lf task list` filter. Never run
`lf desktop view` here: it opens `loopflow://` in the installed app, which
registers the disposable repository in the real `~/.lf` and starts real
conversations there.

Recovery: an auto run's carrier is the command as typed, with parent 1:
`lf task auto on <id>`, or a bare `lf task auto on` from the
checkout (find it with `ps -axo pid,ppid,pgid,command | grep <LF_HOME's bin>`);
`kill -9 -<its pgid>` and its
headless `claude`; the server's pass carries it again as `lf task run <id>`
within about a minute. The killed Graph process stays `Current` (and the
Task reads `moving`) until the hourly prune; `lf machine prune`
settles it now.

With no server: stop the server, then kill the carrier. The next
`lf task status <id>` reads "Run died at <step> · no server runs here to start
it; `lf task run <id>` starts it again". That read is the only one: the
`lf task run` (and later reads) bring a server up, after which the sentence is
"… · starting again" until the pass carries it.

# Upgrade from a released lf

Copy the installed binary out (`cp $(readlink -f ~/.local/bin/lf) …`), make a
fresh repo (local Git identity, a bare origin, `graph:` naming a graph with a
headless step) and `LF_HOME`. Keep the provider stand-in on the private `PATH` and
start the released `lf server run` detached before launching any Task. Run
`lf checkout <id>`, then the released `lf ship <id>` detached with the marker
file holding its step (it blocks its
caller), and `lf task hold <id>` on another. Stop the released server and
start the branch's on the same home. The branch migrates it in place on the
first read; copy the home first to repeat the check. The released lf has no
`ship-review` skill: name the upgrade graph's interactive step `human-review`.
A Task left waiting there ships after the upgrade (the captured Graph carries
its skill). A released `lf task hold` on a Task that never took a Graph comes through as
`auto: off by <person>`; with no checkout, `lf task status` shows it on the
`Not checked out on <machine> · auto: off by …` line. A repository graph that
still names `human-review` then fails a branch run with "skill not found:
human-review"; give the home a second graph naming `ship-review` (push, merge
into existing checkouts) for the runs after the upgrade. A released ship's row
names nobody (`person` NULL), so the backfilled setting reads `auto: on,
<time>` with no "by"; a released hold names its person.
