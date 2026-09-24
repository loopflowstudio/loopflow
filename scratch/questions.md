# Open decisions and limits

## Implemented interpretation

- New conversation follows visible repo/Wave/Project/Task scope and honors lf's configured destination. It starts a fresh conversation without replacing existing Sessions.
- All work uses repository launch context; returning to prior details restores that subject.
- New terminal is an ordinary shell grouped by checkout. Both split levels remain independent.
- Main-view owns unified navigation; unmatched Sessions remain reachable. No one-current-conversation policy is enforced.
- Shell attachment uses actual PTY evidence, never cwd/title inference. Clients predating this marker need a fresh launch before they can report local attachment.

## Open product decisions

- Whether to add an explicit New task/worktree action alongside New conversation. The user's latest scoped-conversation proposal reopened automatic creation.
- Whether and how an exploratory checkout becomes a Task without replacing its files, conversation, or terminals. Actual Tasks require an existing Project.
- Conversation lifecycle/cardinality: general subject conversations must coexist with manual agents and independent exploratory worktrees.
- App-relaunch layout/process persistence, beyond retention within a window's lifetime.
- Integrated ownership is Product → Desktop → LOO-291; do not create a second Task.

## Remaining live proof

- Test an actual configured app launch and a terminal-configured provider launch through the new action; current native checks use harmless commands and fixture records.
- Installation and engbot cleanup are recorded in the integration handoff. Keep list historical: it owns abandoned LOO-276 and must not be hard-deleted.
- LOO-291's separate external-workflow trial and broader acceptance criteria are not completed by integrating its committed navigation here.


## Integration decisions — 2026-09-23

Latest human steering supersedes the older Task-first/cardinality assumptions:
use fresh scope-aware New conversation and separate New terminal, preserving all
existing Sessions. lf-new is now integrated code, including nested worktree
layouts and live PTY attachment, not merely a design snapshot. Bounded lifecycle
and explicit Task creation/adoption remain open; do not enforce one-current
cardinality by hiding or stopping clients. See
`lf-new-implementation/integration.md` for reconciled proof and remaining scope.

The supplied installed screenshot establishes a real appearance defect. The
integrated candidate has readable light/dark captures; the installed app has not
been replaced by this pass. Its New conversation confirmation remains pending.
Do not equate a static capture or fixture with that configured interaction.
