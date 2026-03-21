## Try it!

```bash
cargo test -p loopflow notion
cargo test -p loopflow pm
cargo clippy -p loopflow -- -D warnings
uv run pytest python/tests/
```

To exercise the live auth path:

```bash
export NOTION_CLIENT_ID=...
export NOTION_CLIENT_SECRET=...
lfq auth notion
lf ops auth notion
```

To point PM ops at Notion:

```yaml
# .lf/config.yaml
pm:
  provider: notion
notion:
  title_property: Name
  status_property: Status
  done_value: Done
  priority_property: Priority
```

Then run `lf ops pm init`, `lf ops pm pull <wave>`, or `lf ops pm status` against a Notion-backed wave. You should see Notion databases/pages participate in the same PM lifecycle as Asana and Linear, with page bodies round-tripping as markdown.

## Intent

Add Notion as the third first-class PM backend for loopflow so teams that track work in Notion databases can use the existing `lf ops pm` workflows without flattening items into plain text mirrors. The change covers the full path: auth, provider dispatch, project/item sync, page-body conversion, config, and provider-specific IDs in wave files.

## Assumptions

- The authorized Notion app has access to the target workspace and can create/search pages and databases.
- The target database schema either uses the default `Name` / `Status` / `Done` / `Priority` properties or overrides them via `.lf/config.yaml`.
- Local auth can bind `http://localhost:19223/oauth/callback`.
- Notion's API behavior around separate page-property and block-body reads remains the same, so `list_items` continues to require per-page body fetches.

## Key decisions

- Treat a Notion item as a real page and sync the description through page blocks rather than a rich-text property.
- Reuse the existing PM provider interface and model team-as-page / project-as-database instead of inventing a Notion-only workflow.
- Keep Notion auth OAuth-only, using the browser flow plus Basic-auth token exchange; no API-key fallback for PM ops.
- Make property-name mapping configurable so the provider works with default and customized Notion schemas.

## Not included

- Rich-text formatting support for Notion comments
- Deep nested block/list preservation beyond one level
- A new PM abstraction layer shared across providers
