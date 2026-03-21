## Try it!

```bash
cargo fmt --check
cargo test -p loopflow notion
cargo test -p loopflow pm
cargo test -p loopflow provider_auth
cargo clippy -p loopflow -- -D warnings
.venv/bin/pytest python/tests/
```

Connect Notion locally:

```bash
export NOTION_CLIENT_ID=...
export NOTION_CLIENT_SECRET=...
lfq auth notion
lf ops auth notion
```

Point PM ops at Notion:

```yaml
# .lf/config.yaml
pm:
  provider: notion
notion:
  parent_page: 32af8f99-...  # optional existing parent page/teamspace for pm init
  title_property: Name
  status_property: Status
  done_value: Done
  priority_property: Priority
```

Then run:

```bash
lf ops pm init
lf ops pm pull <wave>
lf ops pm status
```

You should see Notion databases/pages participate in the same PM lifecycle as Asana and Linear, with page bodies round-tripping as markdown. If `notion.parent_page` is set, `lf ops pm init` now creates projects under that existing parent instead of creating a fresh top-level page.

## Intent

Add Notion as a first-class PM provider instead of a side path: local auth, stored credentials, PM bootstrap, pull/status flows, and item body round-tripping all work through the same abstractions already used for Asana and Linear.

## Assumptions

- Reviewers who want to exercise the live OAuth path have a Notion OAuth app and can set `NOTION_CLIENT_ID` / `NOTION_CLIENT_SECRET`.
- The authenticated Notion integration has write access to the configured `notion.parent_page` when that override is used.
- Notion schema overrides (`title_property`, `status_property`, `done_value`, `priority_property`) match the target database when syncing an existing project.

## Key decisions

- Reused the shared `PmProvider` seam so Notion participates in init/pull/status/sync without bespoke orchestration.
- Stored Notion item bodies as markdown locally and translated them through `notion_blocks.rs` to preserve richer page content.
- Reused provider-auth storage for Notion OAuth so `lfq auth notion`, `lf ops auth notion`, and PM ops all share one credential source.
- Extracted shared OAuth callback helpers while adding Notion, which keeps the Asana/Linear/Notion browser flows structurally aligned.
- Made `pm_init` honor `notion.parent_page` so bootstrap behavior matches the rest of the Notion client config.

## Not included

- Live credential-backed OAuth verification in this branch.
- Notion README/supporting-doc sync beyond roadmap/project PM flows.
- Conflict merging for concurrent page-body edits.
