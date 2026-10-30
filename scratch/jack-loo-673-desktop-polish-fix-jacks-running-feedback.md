
## Batch 2 intake (Oct 30)
13. Sidebar drag: dragged row lingers ~0.5s after release before the Task moves. Fixed in-branch: gesture drag, row follows pointer, lands on release.
14. Abandon leaves the left pane in a weird state ("Open Task is outside this list"): abandonTask keeps the abandoned Task selected and never removes its row early.
15. Unassign leaves the Task in the left pane (LOO-312): Working list (selection.rs) ignores ownership while the Wave board moves unassigned Tasks to Backlog. Jack's Desktop unassign of LOO-312 recorded no entry; CLI unassign at 11:55 Oct 30 succeeded.
Relation to LOO-675 (#1773, open, CI red, conflicting): it moves abandon/unassign writes in-process; list semantics and selection are unchanged.
