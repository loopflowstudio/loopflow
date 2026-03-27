# Dependency Sync

**Finish line:** `needs:` declarations on wave items round-trip as native dependencies in Asana, Linear, and Notion. A human looking at any of the three tools sees which items block which, without consulting the repo.

## Context

Wave items declare dependencies with `**Needs:** wave/item-name`. This is the source of truth — it lives in the markdown, version-controlled, written by humans, agents, or the garden.

All three PM providers have native dependency concepts:
- **Asana:** task dependencies (`POST /tasks/{id}/addDependencies`, `dependents` / `dependencies` fields)
- **Linear:** issue relations (`type: "blocks"` / `"is blocked by"`)
- **Notion:** relation properties between database pages

The sync should push `needs:` into the native representation and pull back additions made in the PM tool. Same pattern as priority sync — loopflow's model is semantic, providers speak their native vocabulary.

## What to build

1. **Parse `needs:` from item frontmatter or body.** Extract `needs:` lines during item read. Each entry is a `wave/item-name` reference. Resolve to the target item's provider ID via `RoadmapItemFrontmatter::id_for(provider)`.

2. **Push dependencies on export.** When `pm_export` or `pm_sync` writes an item that has `needs:`, set the native dependency/relation in each configured provider. Clear stale dependencies that were removed locally.

3. **Pull dependencies on import.** When `pm_pull` reads items, check for native dependencies. If a dependency exists in the PM tool but no corresponding `needs:` exists locally, add it. Same conflict rule as everything else: pull path = PM wins.

4. **Cross-wave references.** `needs: model/3-wave-modes` references an item in a different wave's PM project. The sync needs to resolve cross-project references — Asana task IDs are global, Linear issue IDs are global, Notion page IDs are global. The hard part is discovering the target item's provider ID when it lives in a different wave's project.

## Constraints

- Don't invent a dependency data model beyond what `needs:` already provides. The markdown line *is* the model.
- Cross-wave dependencies are the common case (macos items needing model items). Single-wave internal dependencies are less common but should work the same way.
- Dependency direction matters: `needs:` means "I am blocked by." Push as "blocked by" in the PM tool, not "blocks."
- Missing target items (typos, deleted items) should warn, not fail.

## Done when

- `lf op pm export` pushes `needs:` as native dependencies in Asana, Linear, and Notion
- `lf op pm pull` adds `needs:` lines for dependencies created in the PM tool
- Cross-wave dependencies resolve correctly across PM projects
- Round-trip: add a dependency in Asana → pull → `needs:` appears → export → Linear and Notion also show it
