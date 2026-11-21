---
extends: builtin
---
## Starting a Task on a private Machine

For a Swift UI change, open [Loopflow Dev](../../swift/README.md) on the live
Machine and use disposable Work. For CLI and server changes, use the branch
`lf` with a [private Machine and repository](../qa/served-launches.md).
Before the first Start, add `.lf/graphs/qa.yaml` (`- cmd: help task` then
`- interactive: demo`), set `graph: qa` in `.lf/config.yaml`, and commit and
push to the disposable repository's origin.
Then `lf task create '<request>' --run --json` (stdin closed) or a served
`task.create {run: true}` admits the Task, holds its conversation and starts
the Graph from setup. Pass the words positionally: `--title` alone records no
request, so the conversation prints no words above `Checkout ready`.

Attach with `lf session resume <session>` in a PTY to read the setup lines.
Setup and server steps are `loopflow::start_timing` lines in
`$LF_HOME/logs/lf.log`, keyed by request ID; the CLI waits for `sync_planning`
after the answer, a served client does not. A `<repo>.ready` worktree sits
beside the Task checkouts; `ls -di` before and after a Start shows the take.
To reopen a conversation after killing its stand-in provider, write a line to
`$CLAUDE_CONFIG_DIR/projects/<cwd with / and . as ->/<thread>.jsonl` for both
the `/tmp/…` and `/private/tmp/…` spellings of the checkout.

A served request needs the whole envelope, `"select": null, "sender": null,
"request_id": null` included: without any one of them the server answers "this
server has no operation `<kind>`". To stop setup on purpose, `chmod a-w` the
directory holding the checkouts before Start (the answer stays `admitted`;
`lf task status` shows "Setup stopped: …"), restore it, and reopen from a PTY
running `sh -c 'tty; exec lf session resume <id>'`: the printed tty tells your
view apart from a held terminal that takes it ("moved to ttysN"). Replacing a
binary a live server runs: `rm` it and copy the new one, or macOS SIGKILLs the
next exec (status exits 137); then restart the private server.
