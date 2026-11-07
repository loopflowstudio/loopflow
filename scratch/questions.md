# LOO-498 — remaining review

Current accepted direction: [rex-workspace.md](rex-workspace.md). Jack Heart
requested ordinary native tab creation plus `lf resume`, and removal of the custom
viewer. Never demonstrate by reopening his active Task or touching its panes.
Prior judgments: `21bbb00329:scratch/questions.md`; passive viewing and its
readiness protocol are superseded.

- Background arrivals show Task/workflow in the conversation slot, preserving
  input ownership; explicit opening runs resume. This is a reversible implementation
  choice, not a request for another review ceremony.
- Native cmux interaction and two-Machine behavior retain only fixture proof.
- New hands-on proof must use separate work. Earlier rendering was visibly broken,
  not accepted. Retain all demo roots and owners; no input settlement or teardown
  authorization has been received.

LOO-717 merged through PR #1811; its Desktop ownership and shared stream
transport are incorporated. Parent review history remains at
`5f4ee78ee9:scratch/questions.md` and `5f4ee78ee9:scratch/loo-717-shell-lifetimes.md`.
Keep this Task’s walkthroughs; parent scratch filenames are independent reviews.

- `lf open --task` on a Task with no checkout: Jack Heart chose “start the task” (November 7) over keeping Task & workflow. Opening now places an open local Task, which claims it like any checkout, then launches the usual layout and `lf resume`. No Graph starts. Repository and Wave openings still place nothing; done and abandoned Tasks keep Task & workflow.
- Auto-Ship: the 12:44:43Z approval was set by `lf ship` in Session 310fb7b6, five seconds after Jack Heart answered “ship it” at 12:44:38Z (November 7). The question put to Jack: “"ship it" and I'll run the final checks and merge, or "wait" and it stays open until you've tried it.” The PR was at 8a8ae78ef. That `lf ship` started this Graph run, 36fcb4c7, and everything since 8a8ae78ef is that run repairing what its demo found. The approval stands until `lf ship --stop`. Earlier passes called its source unrecorded; comment 97a02520 and the conversation record it. Jack has not tried the later tree by hand.
- Broad `lf open` starts no agents (November 7 demo, [workspace-demo.md](workspace-demo.md); built). An explicit Task or named Session resumes anything. A repository or Wave opening follows the Session states every other client uses; see the third concept review in [rex-workspace.md](rex-workspace.md). The Wave's own conversation follows the same rule.
- A Task with no conversations and no resolvable harness now fails its receipt and creates no tab, rather than a pane that exits. A Task with an earlier conversation skips the check because resume reuses its saved agent. A pane starting a Task's first conversation receives `--agent` from `lf -a`; an ended Session keeps its harness. The `Resume::Import` path stays untraced because this rule never reaches it.

One product choice is open, found November 7 and not blocking: should `lf open --task` on an attached conversation act the same on every host? Rex, Herdr and cmux move it (`Resume::Any`). tmux and plain terminals refuse and print `lf resume <id>`, as they did before this work. Either they also move it, which removes `elsewhere`, or native hosts refuse too, which changes the accepted “explicit Task resumes anything”. Kept as built; `docs/workspaces.md` states the split.

The three listed before the third concept review closed on existing direction:

1. A broad open finding a conversation live elsewhere: it reconnects a `detached` Session and leaves an `active` one where it is. Neither earlier option. `session_actions` already says an active Session's Open is unavailable, “use Take back to transfer it”, and that “unsent text in the previous terminal is not transferred”. The Task says to reuse LOO-672's Session semantics and preserve unfinished input; steer 288a39bd says “Preserve input authority”. Built. If Jack wants a broad open to take conversations back anyway, that reverses one predicate.
2. Auto-Ship: stands, per the entry above.
3. Task & workflow does not keep a worktree. Comment 342b4e92 (November 5): “retain the worktree for an open LF viewer of an associated Task Session, not a random shell in its directory”, and “a Rex tab left after viewer exit is not a separate hold”, superseding “the stronger retained-block interpretation”. LOO-717's accepted experience: “Only Task Session viewers retain the checkout.” Built, and `docs/workspaces.md` says so.
