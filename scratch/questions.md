Pre-demo choices that still hold:

1. Pause applies to open, started Tasks only (refuses `unstarted` Backlog and terminal Tasks); `unassign`, `complete`, `abandon`, and `reopen` clear the pause hold.
2. Explicit `lf task unpause` (with `--run` shortcut) lifts the hold; `lf task run`, `lf task auto on`, carrier and limit recovery respect it. Conversations (steers, primary recovery) do not.
3. Descendant-aware worker teardown is shared by `pause`, `interrupt`, and `move`.

## Implementation choice

Prove worker teardown through `lf task interrupt` first. Use the worker’s OS session to find builds whose launching shell already exited.

Use a separate per-Task cleanup lock while signaling. Run admission can then report the committed hold immediately, while a concurrent unpause or repeated pause waits for the first cleanup.

This Codex invocation's tool shell was PID and SID 98288, a child of provider PID 62413 in SID 671. A live tool launched that way needs its own execution identity. The Claude worker-session fixture cannot establish that Codex stopped such a build.

Carry the original pause time and author in a move. Keep the source hold until the destination acknowledges arrival so claiming back a failed move keeps the Task quiet. A sender from before pause existed omits this field.

Use the CLI pause receipt as the served answer. A plain unpause answers null. With `--run` it answers the same run receipt as `task.run`.

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

Use the recorded interrupt outcome for status. Say the step is stopped while
paused, since an interrupt may also have occurred before the hold. Carrier
recovery takes the same admission lock as pause through its restart count and
launch, closing the gap between checking the hold and spawning the carrier.

On unpause the action and lifecycle update immediately. The list keeps the row in place until the Machine returns the group for its saved cursor.

## V4c

Keep the fixed checkmark from the V4c prototype. A merged PR accents Complete immediately. A post-merge step that is still running or paused keeps its own control.

Bare CLI unpause still lifts the hold. The apps send only smart unpause, from the row or the workflow control.

The phone’s metadata menu opens recorded conversations. The Mac’s independent New conversation starts a local Task shell. The phone’s held-terminal API can continue a recorded Session, but it has no matching independent-chat preparation operation.

## Stack sync with remote workspaces

Keep this Task's scratch files through the parent branch's scratch clear. On
the phone, the Session pane opens the primary conversation by default.
Choosing a recorded conversation in the metadata menu switches to that pane.
The Shell pane keeps its own draft.

## Position-aware unpause

A separate `task run` can see a different cursor after the hold is released.
The detached `task unpause --run` owns the admission lock from the decision
through placement. At a waiting step it reports null before preparing a run.
The Swift decoder and landing-stop button travel with the wire change so both
apps can open a Task that reads `wait`.

## Open questions (beyond LOO-1078)

- **Holistic session-vs-worker availability:** A paused Task keeps its primary Task Session open, so sidebar rows can currently show `Available · inferred` while in the `Paused` Working group. The Task Session and the Graph worker are separate concepts, so leave `Available` intact in `LOO-1078` and think through availability semantics holistically in a dedicated design pass.

## C1 track (Revision 5)

The chip text is Rust's `WorkflowControl.label`, so Rust words it: running
`build · implement` (the step alone when it is top-level), a human step
`ship-review · next: deliver`. Paused, Pausing, Waiting, Run and Done keep
their words. The bar no longer names the Graph; the track's hover shows Rust's
sentence (or setup's reason).

A pending Run or Interrupt changes no row; the chip reads Starting… or
Interrupting… until a reading covers the answer. Auto patches the switch and
the row's auto mark, and moves an Active Working row to Shipping; turning it
off leaves the group for the Machine to say.
