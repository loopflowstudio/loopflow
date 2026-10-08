# LOO-418 implementation assumptions

2026-10-07 · Jack Heart rejected the premature demo stop. Complete the larger
lifecycle changes on PR #1499 before demo. The launch/alias-only milestone was
the agent's scope reduction. Accepted decisions live in [the design](make-a-task-up-to.md).

The remaining proposals are implementation defaults, not new decisions attributed
to Jack or reasons to stop before building the full change:

- Use the proposed polling limits and filing CLI; verify retry and interruption.
- Remove redundant `-c` and serial `--next` with the old PR-chain implementation.
- Surface due follow-ups on the owning Wave's next operation. Filing installs no
  timer; an unattended check must already be authorized in its brief. Exact-time
  wakeups are outside scope. Present this behavior in the complete demo.
- `--design PATH` transfers the child-specific plan without replacing newer
  child work. Preserve scratch isolation and stacked sync.
- LOO-385 overlaps. Recheck before any parallel implementation or external
  disposition; no closure or transfer is authorized by this work.

Slice 1 and the completion alias are implemented. Slices 2–4 are current work,
not deferred follow-ups. No landing or installed-Home migration is authorized.
