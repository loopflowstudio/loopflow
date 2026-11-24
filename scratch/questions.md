Pre-demo choices, superseded by the revision at the top of the plan (recorded in `scratch/plan.md` and `scratch/concept-review.html`):

1. Pause applies to open, started Tasks only (refuses `unstarted` Backlog and terminal Tasks); `unassign`, `complete`, `abandon`, and `reopen` clear the pause hold.
2. Explicit `lf task unpause` (with `--run` shortcut) lifts the hold; `lf task run`, `lf task auto on`, `lf resume --task`, recovery, and steer wakes respect the hold while paused.
3. Pause closes the primary Session via `Departure::Paused` (retaining thread and history) and lets the server bring it back on unpause unless the person had already quit it beforehand.
4. `TaskSection::Paused` sits outside `Working` and `Backlog`, wired into `TaskList.paused` on iPhone and a discoverable Paused view in Mac's sidebar and Wave board.
5. Descendant-aware worker teardown is shared by `pause`, `interrupt`, and `move`.

## Implementation choice

Prove worker teardown through `lf task interrupt` first. Use the worker’s OS session to find builds whose launching shell already exited.

Use a separate per-Task cleanup lock while signaling. Run admission can then report the committed hold immediately, while a concurrent unpause or repeated pause waits for the first cleanup.

This Codex invocation's tool shell was PID and SID 98288, a child of provider PID 62413 in SID 671. A live tool launched that way needs its own execution identity. The Claude worker-session fixture cannot establish that Codex stopped such a build.

Carry the original pause time and author in a move. Keep the source hold until the destination acknowledges arrival so claiming back a failed move keeps the Task quiet. A sender from before pause existed omits this field.

Use the CLI pause receipt as the served answer. A plain unpause answers null. With `--run` it answers the same run receipt as `task.run`.

Both app controls send plain unpause. The Mac subscribes to the selected section. The server looks up checkout and Session details for each matching Task, so including held Tasks in every Working read would do unnecessary work.

Use a real local Git merge to exercise a draining land. Its pre-merge-commit hook can run pause while the command is inside the mutation.

Leave separate-session Codex jobs outside signal authority until the spawn can identify the tool session. On 0.171.0 the live terminal exposed no OS PID. The zsh startup experiment missed an explicit sh tool. The plan records the two orphans and the provider replies.

A source-side pause during a move belongs to the sender until it records the sent payload. After that, pause routes to the destination. Exercise the transfer with the private two-Machine fixture in `machine_commands.rs` and pause from the source's pre-push hook.

Use the existing live-agent index for both worker selection and duplicate PID/birth checks. With 10,000 Tasks and 100 live agents, both query plans used that index, and worker selection looked up captures, Graph steps, and Graphs by primary key.

Use the existing batched latest-Graph read for a single Task status or interrupt. Only the board constructs the full Graph detail it returns.

## Stack sync

Resolve against LOO-1063 using its current Graph ownership, remote Task
projection, and Working order. Keep the implemented pause lifecycle and the
Task's scratch notes during the sync. The post-demo changes in plan.md are
the next implementation slice.

- `cargo test -p loopflow --test integration machine_commands::served_pause_holds_the_remote_cursor_and_status_names_unpause -- --exact` passed on macOS. Gate and CI run broader suites.
- `swift build --package-path swift --target LoopflowMac --jobs 2` built the macOS target with warnings in existing code.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.

## Conversation slice

Apply ordinary conversation recovery and steer delivery during a Graph hold. This includes a stopped primary receiving an addressed steer. Keep list membership and Graph presentation for the next slice.

## Working groups

Keep saved order and status sorting inside each Working group. A drop reorders within its group. For an unselected XOR, a reachable human step before landing keeps the row Active. A Working Task without an observed Graph stays Active unless auto is on.

Automatic verify cannot return across ship-review. Remove that outer loop and file a linked fix-forward Task for a failed merged claim.

On unpause the action and lifecycle update immediately. The list keeps the row in place until the Machine returns the group for its saved cursor.
