Read when the change touches `lf task pause`, `lf task unpause`, worker teardown or what a paused Task reads.

# Pausing a real build

A real Claude worker running real `cargo` shows teardown better than a stand-in.
Keep the real `HOME` (Claude's login lives there) and give only `LF_HOME` a
private directory. Strip every inherited `LF_*` and `CLAUDE*` variable in a
wrapper (`CLAUDE_CODE_CHILD_SESSION` leaves the worker without a transcript):

```zsh
exec env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  $(env | grep '^CLAUDE' | cut -d= -f1 | sed 's/^/-u /') \
  LF_HOME=/tmp/lfqp/home LF_BIN=$BRANCH_LF $BRANCH_LF "$@"
```

Make a repository with a bare origin, `planning: {provider: git, personal: true}`,
`agent: claude` and `graph: qa` (`- compile` then `- interactive: demo`). Don't
call the skill `build`: a builtin Graph has that name and the run fails with
"graph cycle detected". Make the skill run
`cargo build --manifest-path slow/Cargo.toml` in the foreground. Give the crate a
`build.rs` that sleeps 240 s. Claude's Bash tool kills a foreground command at
its timeout, so tell the skill to use 10 minutes. Start `lf server run` before
creating Tasks.

Task checkouts are made from the commit that was current when the Task was
created. After you push a Graph fix, run `git merge --ff-only main` in each checkout.
Two Tasks running `lf task run <id>` one after the other put two
`build-script-build` processes up within about 30 s (`pgrep -f build-script-build`).
Each process chain is worker `claude -p` → `zsh -c` → `cargo` → `build-script-build`.
The primary `claude --name <id>` runs beside it. macOS `ps -o sess` prints 0, so
check survivors by PID.

After pause, `lf task status` shows the saved Graph step and the same primary
Session. Send a steer to that conversation and check that it receives the turn
while the Graph stays held. In `lf task list --json` the owned Task has
`section: working` and `working_group: paused`. An auto Task beside it stays
under Shipping. Both count toward Working on Mac and iPhone.

The served answer to `task.pause` can read `pausing (Graph run … is finishing)`
while the CLI waits and prints `paused`. Read `lf task status` a moment later.

# Working groups and the open conversation

Don't name a repository Graph `qa`: the builtin `build` Graph's `- qa` step
resolves to it, and `lf graph show code` fails with "loop at concept-review
returns across interactive step …". Call it `hold` (`- interactive: demo`,
then `- compile`). Then one Task run to `demo` reads Active, one run on to
`compile` and paused reads Paused, and one with `lf task auto on` reads
Shipping in `lf task list`.

Each primary `claude --name <id>` first stops on Claude's "Adding a new
directory" dialog for the main checkout, so a steer stays undelivered. Answer
it with `lf resume --task <id>` in a PTY: wait about 6 s, send `ESC [ B`,
then Enter, and disconnect. The first turn then reads the steer as direction
on record (`lf session steers <session> --json` shows `delivered_at`).

With the screen locked, the `LOOPFLOW_UI_TEST_MODE=live` snapshot in
[desktop](desktop.md) shows the sidebar groups and the Task bar. Rebuilding
`lf` while the private server runs makes Desktop answer "another executable
still answers": restart the server on the new binary first.

To select a Task in the live-mode Dev bundle, open `loopflow://<repo name>/task/<id>`
with `open -g -b <bundle id>` a few seconds after launch. `lf open --task <id> --link`
prints a title line before the URL, so don't pass its output to `open`.

# A landing stop

To see Active versus Shipping with a merge ahead, add a Graph `landing`
(`- compile`, `- cmd: land`, `- cmd: help task`), push it, and start a Task
with `lf task create '<words>' --run --graph landing` (`--graph` needs
`--run`). With auto unset the running Task reads Shipping, which is correct
because an unset auto merges. Only `lf task auto off` makes the landing a
human stop, and the Task then reads Active. After compile, the Task waits at
`land` and stays Active. A file written into the checkout shows the
uncommitted-work condition after unpause.

# Stopping the private home

Any branch `lf` command against the home registers a launchd job,
`com.loopflow.server.<socket hash>` (plist at `$LF_HOME/server.plist`), so a
killed `lf server run` comes back. The server's recovery also restarts killed
primaries, and their hooks restart the server. Run `launchctl bootout
gui/$UID/com.loopflow.server.<hash>`. Then check each PID's `LF_HOME` with
`ps eww` and `kill -STOP` the holder, the server and every
`claude --name <id>` together before sending TERM and CONT.
