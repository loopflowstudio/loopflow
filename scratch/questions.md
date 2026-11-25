# Open questions (LOO-1101)

- Assumption taken (for review): binding never moves a conversation's directory,
  including out of main and Wave checkouts. This reverses the directory half of
  #1807 (LOO-582). The narrower variant (move only out of shared checkouts) was
  passed over.
- Assumption taken (for review): `lf session bind` prints one factual line when
  the Task already has a different checkout (`Conversation stays in …; <TASK>'s
  checkout is …`), and stays silent about directories otherwise.
- `lf resume` help says "With none for this scope, it starts one and says so",
  but an unowned linked worktree with no conversation refuses with "This
  worktree has no associated work conversation." Starting one there would be a
  separate behavioral change. Left out of this Task.
- `lf task move` (`moves::send_task`) checkpoints `task.worktree`, so if a Task
  has a split primary conversation with uncommitted edits in another worktree,
  those edits do not travel with the move. Left as a separate follow-up.
- Sessions a past bind already moved keep the rewritten `cwd`
  (`session_94d9963b7fd74dacbe01b65c99f7deb5` → `manabot.mtg-388-getting-into-a`,
  launched in `manabot.academy-test-drive`). No repair is planned; is a
  one-time fix of that row wanted?
- The agy process for that Session ended at 2026-11-25T08:29:39Z with no exit
  code and the recovery pass restarted it in the Task's worktree. Cause of the
  exit not investigated.
- When adopting a worktree with `--run`, choose the last-opened conversation
  eligible for that Task. This follows the existing checkout selection rule
  even when a more recently opened Session stayed there after binding elsewhere.
