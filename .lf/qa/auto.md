Read when the change touches `lf ship`, `lf task auto`, the auto setting or a Task run's landing step.

# Auto on a private Machine

`lf-world build task-review <dir>` gives five Tasks; their IDs change with each
build (`lf task list`). `<dir>/github/pulls/<n>.json` names each PR's branch:
one Task's PR is merged by hand, others are open, draft or unavailable
(`<n>.unavailable`), one has none. The world's `gh` answers only reads, so a
`land` step succeeds only on the merged one ("Pull request already merged")
and fails elsewhere ("could not resolve GitHub owner/repo", "no active PR"); a
failure there still shows the run got past the auto check. Start the branch's
`lf server run` detached first (the world's `launchctl` refuses bootstrap).

Graph: `.lf/graphs/qaship.yaml` with `- implement`, `- interactive:
human-review`, `- cmd: land`, `- interactive: verify`; commit, push, then
`git merge origin/main` in each Task checkout (the graph is read from the
checkout). For a run with no interactive step ahead, `- implement` then
`- cmd: land`, on a fresh `lf task create '<words>' --json`.

Headless steps: a `claude` ahead of the world's on `PATH` that, for `-p`
with no terminal, reads one line, waits while a marker file exists, then
prints `{"type":"result","subtype":"success","result":"done",…}`; otherwise
it execs the world's. Touch the marker to keep `implement` running while you
`lf ship`/`lf task auto off` mid-run; remove it to let the run reach the next step.

Read the setting with `lf task status <id> | grep -E 'auto:|Graph q'` and
`lf task status <id> --json` (top-level `auto`; `lf task show <id> --json`
carries it as `execution.auto`). Served: hello from
`rust/loopflow-execution/tests/fixtures/dto/server/hello.json`, then
`{"type":"request","id":…,"operation":{"kind":"task.ship","directory":…,
"issue":…},"select":null,"sender":null,"request_id":"<uuid>"}`; `task.ship`
takes nothing else (a never-run Task takes up the repository's graph), and
`task.auto.off` takes `"reason":null` beside them and answers `{issue, was_on}`.
`lf task auto on` is `lf ship`: check both spellings on a Task waiting at
`human-review` (each stands in there) and on a never-run Task (one Graph run).

Selection state `auto` is a Dashboard expression (`lf desktop dashboard query
'Auto' 'state:auto'` prints JSON), not a `lf task list` filter. Never run
`lf desktop view` here: it opens `loopflow://` in the installed app, which
registers the world's repository in the real `~/.lf` and starts real
conversations there.

Recovery: a shipped run's carrier is `lf ship <id>` with parent 1 (find it with
`ps -axo pid,pgid,command | grep 'lf ship'`); `kill -9 -<its pgid>` and its
headless `claude`; the server's pass carries it again as `lf task run <id>`
within about a minute. The killed Graph process stays `Current` (and the
Task reads `moving`) until the hourly prune; `lf machine prune`
settles it now.

# Upgrade from a released lf

Copy the installed binary out (`cp $(readlink -f ~/.local/bin/lf) …`), make a
fresh repo (local Git identity, a bare origin, `graph:` naming a graph with a
headless step) and `LF_HOME`. Keep the world's `bin` on `PATH`: its `launchctl`
refuses, so the released lf registers no launchd job; start the released
`lf server run` detached instead. `lf checkout <id>`, then `lf ship <id>`
detached with the marker file holding its step (released `lf ship` blocks its
caller), and `lf task hold <id>` on another. Stop the released server and
start the branch's on the same home. The branch migrates it in place on the
first read; copy the home first to repeat the check. A released ship's row
names nobody (`person` NULL), so the backfilled setting reads `auto: on,
<time>` with no "by"; a released hold names its person.
