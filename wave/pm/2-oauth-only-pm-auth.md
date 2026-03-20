---
linear_id: af05f722-57f8-46a9-b64c-ceef04394f20
---
# 08: OAuth-only PM auth

**Finish line:** PM providers use browser-based OAuth through the shared auth surface, and PM commands no longer rely on API-key/PAT setup paths.

Notion should not arrive on top of a mixed auth story. Before adding another provider, clean up the existing PM auth surface so Asana, Linear, and future Notion all connect the same way.

## What to build

1. Remove PM-provider API-key setup flows from `lfq` / `lf ops`.
2. Make PM sync load stored OAuth credentials rather than PM-specific env var API keys.
3. Use the existing Asana/Linear broker path as the baseline and add any missing CLI / route cleanup so the PM experience is consistently browser-connect first.
4. Leave model-provider API-key behavior alone; this item is about PM auth only.

## Constraints

- Keep `lf ops auth` / `lfq auth` as the only local auth surface.
- PM sync should not silently fall back to `ASANA_ACCESS_TOKEN`, `LINEAR_API_KEY`, or future `NOTION_API_KEY` paths.
- This is a prerequisite for Notion work, not a side quest.

## Done when

- Asana and Linear PM flows connect via OAuth
- PM commands read stored OAuth credentials
- PM API-key setup flows are gone
- The path for adding Notion auth is obvious and consistent
