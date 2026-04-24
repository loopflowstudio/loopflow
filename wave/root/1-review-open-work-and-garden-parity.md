---
asana_id: '1214270115593678'
---
# Review-open-work and garden parity

**Finish line:** `review-open-work`, `garden`, and the `govern-*` flows produce one status language, one set of surfaces, and one morning ritual. Manual review is not a separate universe from automated status meetings.

## Context

Today there are two families of status work:

- **Manual** — `review-open-work` walks branches, PRs, worktrees, and waves on demand
- **Automated** — `garden`, `garden-act`, and the `govern-*` flows run on cadence and produce observations, mutations, and calibration pressure

They overlap heavily, but they still feel like different systems. Different wording, different artifacts, different places to look. That split costs trust.

## What to shape

- **Shared signal vocabulary** — wave health, attention items, mutation proposals, calibration notes, shipped work
- **Shared surface contract** — the runboard should show overnight automation and a manual refresh pass side by side
- **Fresh-on-demand review** — running `review-open-work` should be able to trigger a current scan/assess pass before presenting the status picture
- **Clear ownership** — `review-open-work` stays human-driven; govern/garden stays automated; the output model is shared

## Daily experience

Morning coffee. Open Concerto. One runboard answers the whole question: what shipped, what stalled, what the garden proposes, and what needs a human decision right now. If the picture is stale, trigger a fresh pass and keep reading the same surface.

## Done when

- Manual and automated status runs emit compatible artifacts
- Runboard can present both without translation or special casing
- `review-open-work` can request a fresh govern/garden pass when needed
- The morning ritual feels like one system, not parallel checklists
