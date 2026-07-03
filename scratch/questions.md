# Open questions / assumptions

## update-wave: stale freshness SHAs (headless call, resolved by judgment)

The analysis `head:` markers and narrative refs pointed at `615729570…` and
`42a663ee` — commits from a pre-rebase iteration of this branch that `lf op pr:
prepare branch` orphaned (neither is an ancestor of HEAD). That breaks the
commits-behind-HEAD freshness math and cites history that no longer exists.

Decisions made:
- Bumped the five `analysis/*.md` `head:` markers to the current tip
  `5d3f965a…`. The analyses already describe the post-conversation-removal
  world, so they are accurate to the tree; only the pinned SHA was stale.
- Replaced narrative `HEAD 42a663ee` references with "reduce's first reduction"
  (identity-stable) rather than another SHA that a squash-land would orphan again.

Structural note for review: pinning SHAs in wave files is fragile on the
`lf op pr` rebase/land workflow — every prepare-branch and squash-merge orphans
them. If freshness tracking matters long-term, consider a marker that survives
history rewrites (e.g. compare against `main`'s merge point at assess time)
rather than a literal SHA in frontmatter.
