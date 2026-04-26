---
asana_id: '1214270115593678'
---
# review-open-work ↔ build/garden parity

**Finish line:** `review-open-work`, `garden`, and the `govern-*` flows produce one status language, one set of surfaces, and one morning ritual. Manual review is not a separate universe from automated status meetings.

## Context

Today there are two families of status work:

- **Manual** — `review-open-work` walks branches, PRs, worktrees, and waves on demand
- **Automated** — `garden`, `garden-act`, and the `govern-*` flows run on cadence and produce observations, mutations, and calibration pressure

They overlap heavily, but they still feel like different systems. Different wording, different artifacts, different places to look. That split costs trust.

## Shared pieces

- **Signal vocabulary** — wave health, attention items, mutation proposals, calibration notes, shipped work
- **Surface contract** — the runboard should show overnight automation and a manual refresh pass side by side
- **Freshness story** — running `review-open-work` can trigger a current scan/assess pass before presenting the status picture
- **Reusable mechanics** — shared scans, shared summaries, shared routing where the data is the same

## What stays different

- `review-open-work` is human-driven and can do deck-clearing tasks like branch/worktree cleanup
- `garden` and `govern-*` are scheduled system passes that keep pressure moving without a human present
- Manual review can choose what to act on immediately; automated review should bias toward reviewable mutation proposals

## Questions to resolve

- What should `review-open-work` keep that scheduled flows should never own?
- Which sub-steps should be shared outright instead of duplicated?
- Is the manual pass best modeled as “refresh then review,” or as a richer wrapper around the same scan/assess pipeline?
- Which artifacts should persist between runs, and which should be recomputed on demand?

## Daily experience

Morning coffee. Open Concerto. One runboard answers the whole question: what shipped, what stalled, what the garden proposes, and what needs a human decision right now. If the picture is stale, trigger a fresh pass and keep reading the same surface.

## Done when

- Manual and automated status runs emit compatible artifacts
- Runboard can present both without translation or special casing
- `review-open-work` can request a fresh govern/garden pass when needed
- The relationship between manual review and automated status meetings is explicit in the docs and the UX
