# Open implementation notes

## The architecture branch landed; this branch is now attach-only

This branch was stacked on `jack-heart/architecture`, so its history carried 26
architecture commits plus 3 attach commits. That architecture work has since
landed on main squashed as `ae1344a57` ("architecture: establish the durable Run
and Steer control spine", PR #1073).

Rebasing the whole branch would therefore have replayed already-landed work
against itself — the first attempt hit conflicts in seven files across
`child_control.rs`, `flowloop/wave.rs`, `harness/codex.rs`, `journal/mod.rs`,
both runners, and `trace.rs`, all of them re-derivations of code main already
has.

The rebase was redone as `git rebase --onto origin/main ddb829c3c`, replaying
only the three attach commits. Main was first confirmed to carry the primitives
attach depends on (`close_review`, `AttentionRoute`, `attention_at`,
`store/durable.rs`, `lf/commands/work.rs`).

Consequence: the architecture scratch docs (`architecture.md`,
`implementation-plan.md`, `research.md`, the review notes) are gone, because
`lf pr land` cleared `scratch/` when that PR landed. `scratch/attach.md` is the
only design doc this branch owns. Do not restore the others; read
`docs/architecture.md` on main for the ratified target contract.

## Review route vs pending attention: the resolved conflict

Main refined the model after attach forked: the attention **route** outlives a
cleared `attention_at`. A parent Steer answers the pending turn without closing
the Review, so `attention_at` goes NULL while the route stays.

Attach's `advance_review_in` had fenced its attention clear on
`WHERE id=?1 AND attention_at=?2`, which was correct under the older model where
`attention_at` always equalled `opened_at`. Under main's model that predicate
matches zero rows in the legitimate post-Steer case, so continuation would fail.

Resolved by keeping attach's `advance_review_in` extraction — the exit
supervisor's `continue_review_if_current` needs to share it — while adopting
main's semantics inside it:

- the flow-position fence (`step_index=?5 AND interactive=1`) is what rejects a
  stale caller, and it returns `InvalidAuthority` so the exit supervisor's
  `Err(NotFound | InvalidAuthority) => Ok(())` arm treats a changed Review as
  stale and harmless rather than an error;
- the attention clear drops the route by `attention_kind IS NOT NULL`.

`store::durable::tests::exit_guard_steers_and_continues_only_the_exact_user_review`
pins exactly this: it Steers (asserting `attention_at.is_none()`), then requires
`close_review` at the current Basis to succeed and empty `user_attention()`.

## Swift: openability no longer requires an attach argv

Main added `&& launch.attentionAt != nil` to `userAttention` (blue means "needs
you now", not merely "routed to you"). Attach separately dropped
`!launch.argv.isEmpty` from `openable`. Both survive: the Review client builds
its own argv in `LaunchTargetLauncher.reviewCommand`
(`lf work review <kind> <id> --continue-on-exit`) and never reads
`attach.argv`, so a User-routed Review is openable whether or not the Launch
carried an attach route.

## Docs

`docs/lf.md` and `docs/conducting.md` conflicted because main and attach both
rewrote the same handoff-era prose. Resolution kept attach's Review vocabulary
(`lf queue`, `lf work review`, no disposition) and preserved main's distinct
facts that attach did not know about: `lf launch list/status/present/handback`
and the opaque-TUI rule that process exit alone does not claim success.
`lf task attach` is retired from these surfaces, per `scratch/attach.md` §5.
