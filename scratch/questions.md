# W2-172 — open questions & blockers

## Blocker (reported, non-fatal): cannot `lf task acknowledge` from this shell
`lf task acknowledge W2-172 --directive 1 ...` fails: installed `lf 0.11.1`
rejects the shared local store at `~/.lf/loopflow.db`, which has migration
`0.11.004_context_launch_work` applied. This branch's source tree only defines
migrations through `0.11.003_child_body_lease`, so building `lf` from *this*
branch would not resolve it either — some other divergent build owns 0.11.004.
Acknowledge is store bookkeeping; the clarify deliverable (design) is computable
without it. Retry acknowledge once the installed binary matches the DB.

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
