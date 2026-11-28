---
extends: builtin
---
## Desktop

Use the [isolated Dev bundle](../qa/desktop.md#loopflow-dev-on-the-live-machine)
and disposable Work. Check launch won't attach to or resize personal terminals
before opening the live Machine. Capture the changed UI and a real update in the
foreground in both light and dark themes, clicking through so choosing or
hovering never moves or resizes neighbors. Report the check as open if it cannot
run safely.

For Session attachment, join an already-running disposable Session on its
owning Machine from a second view. Check shared output and input, then close
and reopen the second view while the original provider keeps running.

When Desktop must work with the installed server, exercise the newer client
with older responses, including an extra field and an unsupported operation.
Verify that the failure stays local and supported reads and actions still work.
Keep the installed server and schema for the live run.

## Starting a Task on a private Machine

For CLI and server changes, use the branch `lf` with a
[private Machine and repository](../qa/served-launches.md).
Before the first Start, add `.lf/graphs/qa-start.yaml` (`- cmd: help task`
then `- break`), set `graph: qa-start` in `.lf/config.yaml`, and commit and
push to the disposable repository's origin. Don't name it `qa`: a bare step
resolves to a Graph before a skill, so it would replace `build`'s `qa` step.
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

## Shared build cache

Drive `scripts/build_cache.py` in an isolated `HOME` and `CARGO_HOME`, keeping
the real `RUSTUP_HOME`. Copy the installed pin's `bin/sccache` to the same
revision path under that `HOME`; `install` then skips `cargo install`. Run the
branch's `.venv/bin/python` directly, since `uv` under a new `HOME` refetches.
A dependency-free two-crate workspace is enough. Give it `[profile.dev]
incremental = false`, like the repository; never read `CARGO_MANIFEST_DIR`
in a crate meant to hit, because that value stays verbatim in the hash. Touch a
source before a build that must start a daemon: a fresh build compiles nothing.
Stop that daemon by the PID `status --json` reports.
