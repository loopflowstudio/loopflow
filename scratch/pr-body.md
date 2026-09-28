Finite-operation plans and review-launch findings were scattered across scratch
handoffs. This documentation contribution preserves their decisions in Wave
memory and identifies which checkouts now own implementation.

## What changes

- Preserve the intended Flow order, review boundaries, finite landing repair and
  closure contract for [Keep Tasks progressing to landing without a resident or watcher · LOO-332](https://linear.app/loopflow/issue/LOO-332/keep-tasks-progressing-to-landing-without-a-resident-or-watcher).
- Preserve recursive Wave/repository responsibility for
  [Operate Waves recursively and assess the repository with VSM · LOO-333](https://linear.app/loopflow/issue/LOO-333/operate-waves-recursively-and-assess-the-repository-with-vsm),
  with Discord continuity and scheduling integration as separate follow-ups.
- Record review-launch recovery requirements without attributing the unknown
  failing executable or interpreting a selected-store lookup miss as deletion.
- Mark source designs as archived after their transfer to dedicated checkouts.

## Delivery boundary

This copy describes the documentation contribution after `ce97003cf`. The branch
also inherits unfinished [Data model: one table per user object · LOO-298](https://linear.app/loopflow/issue/LOO-298)
and is not ready for publication against main. Settle that base or isolate this
contribution before using this copy. No runtime capability or installed acceptance
is claimed; linked source checkpoints were verified locally, not remotely.

## Checks

Local Markdown targets and whitespace pass. The changed-aware gate selected
architecture only but stopped when resource measurement timed out. Direct
architecture validation fails on unmapped `lf catalog`, `auth_browser_bindings`,
and stale vocabulary in an ignored scratch stash. No behavioral suite was
selected for this documentation contribution; destination Tasks retain their
runtime and human-demo obligations.
