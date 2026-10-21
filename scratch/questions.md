# Open questions (LOO-553)

Recorded by the headless kickoff on 2026-10-21; Jack Heart to confirm.

1. **repo-operate keeps its cleanup duties.** Assumed yes: cleanup is already
   limited to your own PRs, branches and worktrees and involves no KRs, new
   work or claiming. Reversing it means deleting "Clear old work state" too.
2. **"Your own Tasks only" is the operating rule.** Assumed yes: operators act
   on Tasks whose owner person is you (or names no person); other people's
   Tasks are listed with their owner and left alone; unowned Tasks are backlog.
3. **Due follow-ups with an authorized unattended check** are now listed only,
   not run. Assumed this falls under "claiming unassigned work"; reversible by
   restoring the exception for Tasks you own.
4. **No code guard on `lf task run` claiming another person's Task.** Assumed
   the skill rule suffices for now; `lf task claim` stays the explicit takeover.
