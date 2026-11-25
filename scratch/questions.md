# Open questions (LOO-1101)

- Assumption taken: binding never moves a conversation's directory, including
  out of main and Wave checkouts. This reverses the directory half of #1807
  (LOO-582). The narrower variant (move only out of shared checkouts) was
  passed over.
- `lf resume` help says "With none for this scope, it starts one and says so",
  but an unowned linked worktree with no conversation refuses with "This
  worktree has no associated work conversation." Starting one there would drop
  a precondition. Left out of this Task.
- Sessions a past bind already moved keep the rewritten `cwd`
  (`session_94d9963b7fd74dacbe01b65c99f7deb5` → `manabot.mtg-388-getting-into-a`,
  launched in `manabot.academy-test-drive`). No repair is planned; is a
  one-time fix of that row wanted?
- The agy process for that Session ended at 2026-11-25T08:29:39Z with no exit
  code and the recovery pass restarted it in the Task's worktree. Cause of the
  exit not investigated.
