# Notion PM provider review

## What was implemented

- Added a first-class Notion PM provider with database/project creation, item CRUD, completion, comments, and markdown↔blocks body sync.
- Added Notion OAuth support to both `lfq auth notion` and `lf ops auth notion`, and wired PM credential lookup to reuse stored OAuth tokens.
- Extended PM config/frontmatter parsing so waves can point at Notion projects and roadmap items can persist `notion_id` alongside Asana/Linear IDs.
- Polished `pm_init` so `notion.parent_page` is honored during bootstrap, and documented that config in the README and validation notes.
- Reduced duplicated OAuth callback code by extracting shared helpers for env-based client credentials, localhost listeners, and callback monitoring.

## Key choices

- **Keep the PM seam provider-agnostic.** Notion plugs into the existing `PmProvider` trait instead of adding a Notion-specific sync path, so ingest/pull/status/init continue to operate through one orchestration layer.
- **Round-trip page bodies as markdown, not plaintext.** The Notion adapter converts page blocks to markdown and back so README-style content survives sync with higher fidelity.
- **Reuse OAuth credentials everywhere.** `lfq auth notion` and `lf ops auth notion` share the same provider-auth machinery, and PM operations read the stored OAuth token instead of introducing a second Notion auth mode.
- **Allow explicit Notion bootstrap parents.** `notion.parent_page` now overrides automatic team/page creation during `lf ops pm init`, which keeps bootstrap predictable for teamspaces and existing parent pages.

## How it fits together

`provider_auth.rs` now knows how to start and complete Notion OAuth, then stores the resulting token in the shared credential store. `ops/pm.rs` resolves the configured PM provider, constructs the matching client (`NotionClient` for Notion), and drives init/pull/status/sync through the shared `PmProvider` trait. `notion.rs` handles Notion database/page HTTP calls, while `notion_blocks.rs` is the markdown/block translation seam used for item descriptions and comments.

## Risks and bottlenecks

- Notion item import is still N+1: list pages, then fetch page blocks per item.
- Page-body updates remain destructive at the top-level block list; concurrent local and Notion edits still resolve last-writer-wins.
- Live OAuth/bootstrap validation still depends on reviewer-provided Notion OAuth app credentials and a reachable local callback port.
- `notion.parent_page` assumes the configured page/teamspace is writable by the authenticated integration.

## What's not included

- Live end-to-end OAuth validation with real Notion credentials.
- README/supporting-doc import beyond roadmap/project sync.
- Webhook-driven or real-time merge semantics for concurrent edits.

## Validation

Passed locally:

- `cargo fmt --check`
- `cargo test -p loopflow notion`
- `cargo test -p loopflow pm`
- `cargo test -p loopflow provider_auth`
- `cargo clippy -p loopflow -- -D warnings`
- `.venv/bin/pytest python/tests/`

Not run locally:

- `lfq auth notion` / `lf ops auth notion` live OAuth path (requires `NOTION_CLIENT_ID` and `NOTION_CLIENT_SECRET`)
