# Slice 1 repair: prove merged continuation crosses the PR boundary

## Problem

The action model proves that a merged `ContinueTask` PR recommends
`StartNextPr`, and runner source rotates every settled non-completing PR. The
review claims the Task proceeds, but no named behavioral test starts from a
durable merged `ContinueTask` PR and observes serial PR N+1. Current comments
also call the explicit completion Feedback a generic “review gate,” which
suggests the deleted implicit state survived.

## Required behavior

- A merged PR whose authored disposition is `ContinueTask` creates or selects
  the next serial working PR without approval/review state.
- A merged `CompleteTask` PR does not rotate unless already-authored follow-up
  work makes completion impossible under the existing explicit rules.
- Use the real store/worktree rotation path at the smallest practical behavior
  boundary. Do not reshape production code solely for a mock or duplicate the
  action model in a test-only helper.
- Rename stale “review gate” comments/reasons to the exact explicit Task gate or
  Feedback checkpoint they mean. Do not change behavior or restore approval.

## Done when

- [ ] A named behavioral test starts with durable merged `ContinueTask` PR N
      and observes working PR N+1 with no approval/review record.
- [ ] Existing completing-PR behavior remains proved.
- [ ] Current Task code contains no comment or user reason implying a generic
      implicit Review state.
- [ ] Focused Task/PR/action/migration tests, format, and Clippy pass.
- [ ] `scratch/feedback-runtime-review.md` records the stronger proof.
