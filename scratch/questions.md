# W2-172 — open questions & blockers

## Blocker (reported, non-fatal): installed `lf 0.11.1` can't touch the store
`lf task acknowledge` / `lf pr open` / `lf commit` store-writes all fail: the
installed `lf 0.11.1` rejects `~/.lf/loopflow.db`, which has migration
`0.11.004` applied. **Mystery resolved:** that migration is `0.11.004_task_pr_ci_state`
from **#916** (merged to main while this Task was in flight). This branch now
rebases onto main *including* #916, so a freshly-built `lf` from this branch
would match the DB — but the *installed* binary is still 0.11.1. Any `lf`
command that writes the store (acknowledge, Task/PR row updates) stays blocked
until the installed binary is rebuilt from ≥#916. Work proceeds via cargo
tests + `git`/`gh` directly; PR bookkeeping the store can't record is tolerated
by this Task's own degradation code.

## Incident (recovered): `lf pr open` rebased mid-flight, then was killed
`lf pr open` (installed 0.11.1) spawned a `claude` agent for PR-copy generation
that hung, and it had already started a rebase onto freshly-arrived main
(#916/#921/#922). Killing it left an interactive rebase paused on a conflict
between PR1 and #916 (both touch reconcile / `current_or_merged_pr_for_branch`).
Recovered by `git rebase --abort` → back to the clean PR1 commit (safe on remote
+ reflog), then a deliberate re-apply of PR1 onto the #916 base, integrating
`head_sha`. No work lost. Lesson: `lf pr open`'s agent-copy + auto-rebase is a
poor fit when the store is already broken; use `git push` + `gh pr create` for
a reviewable PR in that state.

## Status: PR1 open as #928 (waiting for CI)
Bounded REST-first + degradation-tolerant observation. Integrated with #916.
Full lib suite green, clippy clean. Next serial PR: carry committed follow-up on
rotate (recover W2-166) + directive/completion race guard.

## Assumptions taken (reversible; simpler path chosen)
- **Merged branch tip = REST `head.sha`.** The follow-up commit range carried on
  rotate is `head.sha..settled_HEAD`, where `head.sha` is read from the bounded
  REST PR fetch. This is correct as long as the post-merge follow-up was
  committed locally and *not pushed* to the already-merged branch (the whole
  point of `lf pr next`). Follow-up pushed onto a merged branch is a degenerate
  case, left out of scope.
- **W2-166 recovery is the acceptance demo, not a code artifact of this Task.**
  This Task ships the reproducible capability (carry committed+dirty range on
  rotate) proven by a W2-166-shaped integration test. Applying it to recover
  W2-166's actual worktree is a one-time operational step in that worktree, run
  once the capability lands.
- **Serial PR order:** PR1 = bounded REST-first reconcile + degraded-observation
  resilience; PR2 = carry committed follow-up on rotate + directive/completion
  race guard. The runner owns rotation between them.
