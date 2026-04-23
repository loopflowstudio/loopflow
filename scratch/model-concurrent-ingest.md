---
status: in-progress
claimed_by: jack-heart.model.20260423_1303
claimed_at: 2026-04-23T20:40:58.91863Z
asana_id: '1213869424664965'
---
# Concurrent Ingest — Ordering Normalization

**Needs:** nothing new (wave-crons shipped; PM claim shipped).

**Finish line:** Local `priority`/`rank` frontmatter mirrors the PM provider's
native ordering so concurrent workers converge on the same picking order.

## What shipped already (prior work)

- PM claim coordination via Linear and Asana: read-then-assign with verify on
  the assignment response. Race window small, failure graceful.
- Frontmatter claim cache: `status: in-progress` + `claimed_by` + `claimed_at`.
- Notion best-effort claim (status → "In Progress"); verified claimant identity
  still open.

## What this PR does

Normalizes the order PM items arrive in so within-bucket ordering survives a
pull. Previously `PmItem.rank` was overloaded (Linear: sort position; Asana:
priority bucket; Notion: priority bucket) and the write path dropped fractional
ordering entirely by calling `frontmatter.set_priority_rank(rank: u32)` which
mapped the first four items to Urgent/High/Medium/Low and the rest to Low.

After this change:

- `PmItem` carries `priority: PriorityBucket` **and** `rank: Option<f64>`.
- Linear's GraphQL query adds `priority` alongside `prioritySortOrder`; the
  bucket comes from `priority`, the rank from `prioritySortOrder`.
- Asana keeps its custom-field bucket; rank is the response index cast to f64.
- Notion extracts the bucket from the `Priority` select; rank is the query
  result index.
- A shared `sort_pm_items` helper sorts by bucket then fractional rank then
  name across all three providers.
- `remote_item_to_document` writes both fields to frontmatter so
  `WaveItemOrder::Frontmatter { priority, rank }` (already supported by ingest)
  has real data to sort by.

## What's still outstanding

- Notion claimant identity — the "In Progress" status can't distinguish which
  worker claimed the page. Would need an Assignee property or equivalent.
- Non-PM wave ordering — local-only waves still rely on manually-authored
  `priority`/`rank` frontmatter or the legacy filename prefix.

## Done when (this increment)

- PM pull populates `priority` and fractional `rank` in local frontmatter
- Linear items past position 3 keep their real priority bucket
- Tests cover the bucket extraction and within-bucket preservation for all
  three providers
