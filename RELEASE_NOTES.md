# v0.14.10

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.10 keeps your conversations when Desktop doesn't. Before this release, closing a repository window, switching Tasks or restarting the Mac could cut you off from a running Session. Getting back took several manual reattaches, and after a reboot you had to find each interrupted Session yourself. Now Desktop keeps each repository's panes and drafts until it quits and reattaches to running Sessions without restarting them. At launch it brings back every conversation a reboot or crash interrupted. Upgrade if you keep several Tasks open at once or have lost a Session to a restart. (#1605, LOO-416)

## Interrupted conversations come back

`lf resume --all` reopens every Session whose execution owner is gone without recording an exit. A reboot, crash or kill leaves Sessions in that state. Sessions you exited deliberately are left alone. Desktop runs the command at launch, so after a restart your conversations are already back.

- `lf resume --all` resumes each Session through the ordinary `lf resume` path, in a new owner with no viewer. It skips Sessions you exited through the provider or detached from with Ctrl+].
- The output lists each Session as resumed, still starting or failed. It also flags any turn the restart cut off. That turn is not replayed.
- It waits up to 20 seconds for each Session's owner to bind. If the owner hasn't bound by then, the Session is reported as still starting.
- Leaving a Session now prints the `lf resume` command that returns to it, on every harness. A resume that fails during launch is logged rather than silently dropped.

## Desktop holds its place

Desktop now treats a repository window as a view onto work that keeps running, not as the owner of that work. Closing the window, switching Tasks or showing a pane again no longer costs you anything.

- Closing a repository window keeps its panes, selected Work and file drafts until Desktop quits. Reopening a repository you have used before shows that content immediately, even while repository lookup is still running or has failed.
- **Resume** in Desktop now does the same thing as `lf resume`. It attaches to a conversation that is still running, or reopens its saved provider history.
- `lf session attach --if-free` attaches only if no other viewer has input control, and never takes control from one. `lf session attach --prepare --json` returns an attach-only command without starting a provider. Desktop uses both to restore panes, so a pane becoming visible never takes input away from anyone.
- CLI requests from Desktop time out after 30 seconds, and canceling one stops only that request. When a change times out, its outcome is treated as unknown and the change is never retried automatically.

## Conversations exist from the moment work does

Creating a Task now saves its primary conversation in the same step. Wave admission does the same for the Wave and repository conversations. Saving a conversation starts no provider and doesn't mark the Task Started. Both happen on the first input reserved by `lf connect`. This replaces the earlier step that prepared the first input in advance.

## Limits

- If Desktop itself dies, its in-app state is lost. The Session, its native history and a repaint come back, but live input and exact scrollback do not. The owner keeps a bounded vt100 text state, not a full terminal snapshot.
- A turn cut off mid-flight is flagged and never replayed. With Codex, leaving during the `after-thread` startup window stops the provider, so a first turn it had already accepted can lose its input.
- Not included: preserving layouts across Desktop installations and keeping a durable record of the intended layout. That work is tracked as LOO-497 and LOO-489. Full acceptance of LOO-416 still needs that integration, plus a measured check that native Sessions survive in an installed build.

## Operational notes

- **Schema migration:** the `session_admission` draft is released with this version. It stores the conversations that Task creation and Wave admission save. Seed fixtures for the previously released schema live at `tests/fixtures/migrations/conversation_admission_released.sql`.
- **New CLI surface:** `lf resume --all`, `lf session attach --if-free` and `lf session attach --prepare --json`.
- **Verification:** new Rust tests cover `resume --all` flagging a cut-off turn and stopping the owners it resumes. New transport tests cover `--if-free` and attach-only preparation. On the Swift side, `WorkspaceRetentionTests` checks that twelve Work layouts and their drafts survive window closure. These are twelve layouts, not twelve live providers. `SessionsStoreTests` and the local Wave agent launcher tests were also updated.