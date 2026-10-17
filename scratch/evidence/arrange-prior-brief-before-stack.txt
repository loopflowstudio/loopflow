# Prior Task brief — historical evidence

Superseded where it conflicts with Jack Heart’s October 16 Arrange brief and the current Task description. Preserved to retain prior obligations and provenance; it is not new direction.

Someone who quits or restarts the Mac app, or whose app crashes, reopens it to the same conversations in the same panes.

Acceptance

* The pane arrangement and which conversation is in which pane are saved per Task and restored.
* Each pane attaches to its conversation's relay on launch.
* Shell panes either use the relay or reopen in the same directory; the design decides.

This is step 7 of [Work on another machine · LOO-394](<https://linear.app/loopflow/issue/LOO-394>). Jack Heart decided on 2026-10-07 that each step is its own Task with its own PR, stacked on the step it builds on, so no step waits for another to land. The design is `scratch/work-on-another-machine-name.md` in this Task's worktree; this Task builds the step marked `This slice` there and nothing else.

Constraints decided by Jack Heart

* Do not lift code, tests, help text or config from herdr or cmux.
* A redesign deletes what it replaces, in the same diff.
* No stable cross-version interfaces and no support for older versions yet.
* No shared resident process; no automatic retry of a turn or Flow; no hidden arguments.
* Stop at a published PR for review in Jack's conversation; do not land.

## Work-tab integration

Jack Heart assigned remaining LOO-427 proof and persistence boundaries to existing owners on October 10, 2026. Persist the accepted Work workspace model: one retained workspace per repository, Wave or Task, identified by shared Work identity, with an ordered open set and selection. Reopen the same repository window across Machines; pane content retains its own recorded execution Machine/path. Build on LOO-427 rather than restore checkout-keyed outer layouts or Session tabs. Distinguish retained layouts and provider history from unsent drafts/live PTYs, and state explicitly what survives quit/crash. LOO-391 owns native in-window retention proof; this Task owns relaunch behavior.

<!-- loopflow-task-start:c54d9d935b15f15303076355b3622c86336ef0106880cd34cb323bbfa4e9ecc0 -->
