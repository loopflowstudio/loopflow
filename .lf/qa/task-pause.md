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
