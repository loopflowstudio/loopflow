Read when the change touches a Task's conversations or headless Work, using lf-world.

# Task conversation lifetime

`cargo build -p loopflow --bin lf-world`, then `lf-world build one-task <dir>`
writes a private Machine whose `bin/claude` stand-in echoes each line; its
printed environment holds `LF_HOME`, the provider homes and `PATH`. Under an
isolated `HOME`, give that home a Git `user.email` first or the build fails at
claiming. Copy `lf` and `lf-world` out of `target/` before a long walkthrough so
a rebuild can't change the binary under its owners. `lf task checkout <task>`
warms the Task's conversation detached; `lf session list --json` maps it to its
Session. Bare `lf task retire` runs one pass over every pending Task. Attach the
Task's Session in a PTY with `lf session resume <session>`, type a line without
Enter as a draft, and close the PTY: the holder keeps it running. Nothing
schedules checkout cleanup in a private home; call `lf task retire <task>` to
exercise deletion (the installed `com.loopflow.cleanup` job runs once a minute).
After `lf task complete`, run `lf task retire <task>` once fifteen minutes have
passed without a viewer and live work has settled. That pass signals the verified
conversation owner directly, even without a checkout; a held Task
conversation ends on the server's notice instead. `lf task abandon` ends only
unviewed conversations; one still viewed then ends at the next
`lf task retire <task>` after its view leaves.

A new conversation's stand-in reads its opening turn from stdin, then the
terminal, as Claude does, but writes no transcript: before resuming one after
its provider ends, write any line to
`$CLAUDE_CONFIG_DIR/projects/<project>/<id>.jsonl` (the ID follows `started`). For a done Task without a checkout, prepare a repository
conversation (`lf session resume --prepare --json` in the repository), complete
a checkout-less Task, then `lf session bind <session> --task <task>`. Binding
claims the Task, which reads `git config user.email` in the repository: with an
isolated `HOME`, set a repository-local identity first. A PTY driver waiting for
text can match the repainted saved screen before the new provider reads input;
wait for text only the new provider prints.

# Headless Work and earlier-build owners

lf-world's `launchctl` stub refuses `bootstrap`, so start the branch's
`lf server run` (stdin closed) before any Task launch there. The Graph is
positional: `lf task run <task> <graph> --json`. Headless steps need the fixture
account above. For a headless step that waits, put a `claude` ahead of the
world's on `PATH` that, when neither stdin nor stdout is a terminal, reads one
line (stdin stays open for steers, so never `cat` it), waits for a marker
file, then prints a stream-json `result` line. A detached launch shows as
`lf task run …` with parent 1 and its stdout on `logs/graph-<id>.log`; `-b` at
a PTY tails unlinked temp files. PTY drivers must drain output continuously:
an undrained PTY blocks the caller on write and looks like a hang.

An earlier build's Session owner: in a fresh home, before the branch opens it,
run the installed `lf session serve --terminal -- lf -i -a claude : '<msg>'`
under `nohup`; it prints the owner's Process ID, and `agent_sessions.transport_lf_process_id`
names it. Write the stand-in's transcript line, then resume with the branch.
SIGSTOP the owner first to check the "still running under an earlier lf"
refusal (SIGCONT afterwards; the pending TERM ends it). The installed
`lf desktop shell <task> --checkout <path> --id <id> --create` in a PTY makes an
earlier-build Desktop shell; its Task needs a checkout (`lf wt create` then
`lf task create --worktree`). Running `open_argv` from
`lf session resume --prepare --json` with `LF_SERVER_STATUS` set to any path
takes the Desktop pane's relayed route.
