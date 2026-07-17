# Open questions / assumptions — ENG-22

## Assumptions (proceeding on these)

- **Target team already on every Project.** Verified via live dry-run for
  `infrastructure` (`[ENG, W2]`) and from the task report for `intelligence`
  (`[SCI, W2]`). Assuming `product` is likewise `[PRD, W2]`. The design adds a
  pre-flight assertion so a violation refuses apply rather than silently dropping
  issues — so this assumption is enforced, not just trusted.
- **Linear preserves completion on a team move**, remapping workflow state by
  type. Medium confidence, not tested live. De-risked by the rollout order:
  Intelligence (16 completed) applies before Infrastructure (64 completed), so a
  bad completed-move surfaces on the small batch. The implementation body does
  **not** apply, so this can't bite during this Task.

## Genuinely open (for the orchestrator, not this body)

- Whether `product`'s Projects are exactly `[PRD, W2]`. If a Product Project is
  single-team or missing `PRD`, the new pre-flight will refuse its apply with a
  named diagnostic — surface it to a human to attach the team, rather than force
  it here.
- Exact wording of the pre-flight refusal message is an implementation choice;
  it should name the Project and its current team set.
