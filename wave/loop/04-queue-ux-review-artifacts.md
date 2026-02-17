---
status: proposed
seq: 4
---

# 04: Queue UX + Review Artifacts

Make landing intent obvious in Concerto and remove merge dependence on tracked scratch docs.

## Estimated implementation size

~350-800 LOC across Swift queue views, API DTO consumption, and PR artifact publishing paths.

## What exists after this step

- Runs tab defaults to queue-first workflow (oldest-first landing path).
- Users can see `ready`, `draft`, `blocked`, `merged`, `superseded` at a glance.
- Review summary is published on PR surfaces (managed comment/body block).
- Ready promotion enforces scratch-clean state without post-merge cleanup commits.

## Queue-first Concerto behavior

### Primary question

UI should answer: **"What should I land next?"**

### Default ordering

- Oldest-first queue at top.
- Reverse-chron timeline remains available as secondary history view.

### Role badges

- Ready to land
- Waiting in queue
- Blocked (rebase conflict)
- Merged
- Superseded (combined)

### Primary actions

- `Open PR`
- `Combine PRs`
- `Resolve Blocked`
- `Refresh PR State`

## UX constraints

- No stale badge rendering; consume live-state-backed DTO fields.
- Keep current run detail/log surfaces intact.
- Preserve accessibility conventions from `VISUAL_DESIGN.md`.

## Review artifact publishing model

### Principle

`scratch/` is ephemeral working memory. PR-visible review context must live in managed PR output.

### Canonical storage

Store review summary content in run metadata.

### Publishing targets

Preferred:

- managed bot comment (idempotent update)

Fallback:

- marker-delimited PR body block:
  - `<!-- lf:auto-review:start -->`
  - `<!-- lf:auto-review:end -->`

### Clobber prevention

- only replace managed block/comment
- never overwrite manual user text outside managed area
- preserve custom PR descriptions

## Scratch-clean gate in queue workflow

Before promoting a run to Ready:

- verify no tracked `scratch/` diff
- if dirty, retain Draft and expose clear remediation status

This keeps GitHub Land button compatible while preventing scratch leakage to main.

## GitHub-first merge compatibility

Landing may occur from GitHub UI directly.

Expected behavior:

1. Merge detector advances queue.
2. Concerto updates queue roles.
3. Review artifact links remain valid.
4. No manual `lf ops` requirement.

## API contract needed by UI

Queue projection fields:

- `queue_role`
- `queue_block_reason`
- `queue_index`
- `superseded_by_pr`
- `combined_pr`

Review artifact fields:

- `review_artifact_status` (`published`, `stale`, `retry_needed`)
- `review_artifact_url`
- `review_artifact_updated_at`

## Test plan

### UI logic tests

- queue ordering displays oldest-first by default
- role badges map correctly from DTO
- blocked state shows actionable path

### Publishing tests

- bot comment updates are idempotent
- marker block updates preserve non-managed body text
- publish failures mark retry-needed without corrupting content

### Promotion gate tests

- scratch diff blocks Ready promotion
- clean state allows promotion

## Rollout strategy

1. Ship API fields and hidden queue-first toggle.
2. Enable queue-first as default once backend roles stabilize.
3. Migrate review summary publish path from scratch doc dependence.
4. Remove legacy UI assumptions tied to reverse-chron-only flow.

## Non-goals

- Full redesign of all Concerto wave surfaces.
- Rich templating framework for review summary markdown.
- Multi-PR parallel reviewer workflow optimization.

## Done when

- Concerto makes queue progression clear in one glance.
- Review context survives rebases/merges without tracked scratch files.
- GitHub Land button remains first-class with no hidden workflow penalties.
