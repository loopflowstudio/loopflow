Read when the change touches `lf ship`, `lf task hold`, Auto-Ship or a Task run's landing step.

# Auto-Ship on a private Machine

`lf-world build task-review <dir>` gives Tasks with PRs: `1545df` (PR #11
merged by hand), `173404` (#12 open), `fcc343` (#13 draft), `adf894` (#14
unavailable), `7ca83c` (none). The world's `gh` answers only reads, so a `land`
step succeeds only on `1545df` ("Pull request already merged") and fails
elsewhere ("could not resolve GitHub owner/repo", "no active PR"); a failure
there still shows the run got past the Auto-Ship gate. Start the branch's
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
`lf ship`/`lf task hold` mid-run; remove it to let the run reach the next step.

Read the mark with `lf task status <id> | grep -E 'Auto-Ship|Graph q'` and
`lf task show <id> --json` (`execution.auto_ship`; `lf task status --json`
has no `auto_ship`). Served: hello from `tests/fixtures/dto/server/hello.json`,
then `{"type":"request","id":…,"operation":{"kind":"task.ship","directory":…,
"issue":…},"select":null,"sender":null,"request_id":"<uuid>"}`; `task.hold`
adds `"reason":null`.

Recovery: `kill -9 -<pgid of the detached lf ship or lf task run>` and its
headless `claude`; the server's pass carries it again as `lf task run <id>`
within about a minute. The killed Graph process stays `Current` (and the
Task reads `shipping`/`moving`) until the hourly prune; `lf machine prune`
settles it now.

# Upgrade from a released lf

Copy the installed binary out (`cp $(readlink -f ~/.local/bin/lf) …`), make a
fresh repo and `LF_HOME`, `lf checkout <id>` then `lf ship <id>` with it.
Released `lf ship` blocks its caller; run it with a timeout, never in a
shell loop that goes on after you stop watching. Released `lf` registers a
real launchd job for the temporary home (`com.loopflow.server.<socket hash>`,
plist under that home): `launchctl bootout gui/$(id -u)/<label>` when done,
or it restarts its server forever. Copy the home before opening it with the
branch; the branch migrates it in place on the first read. Released rows
name nobody (`person` NULL), so the backfilled mark reads `Auto-Ship: on,
<time>` with no "by".
