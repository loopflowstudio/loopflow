---
asana_id: '1213717741038313'
linear_id: 704e334b-2a4d-47de-83e7-87d35116ee5c
---
# 11: Notion task parity after the model redo

**Finish line:** after the bucketed-priority redo, OAuth cleanup, README sync, and supporting-doc import land, Notion task databases can join the same `pm init` / `pm pull` / `pm sync` lifecycle as Asana and Linear.

Notion should come after the shared planning model is corrected and after its doc-native advantage is proven. Otherwise we risk adding a third provider on top of the wrong abstractions.

## What to build

### Notion task adapter

1. Implement `PmProvider` for Notion's task-shaped database model.
2. Map databases/data sources to the project surface and pages to items.
3. Use a priority/status schema that matches the new shared bucket model rather than exact rank.
4. Store item descriptions in a deliberately simple page-body representation.

### Provider wiring

1. Add `Notion` to `PmProviderKind`, `RoadmapItemFrontmatter`, wave PM config, and provider construction.
2. Route `ops/pm.rs` through the existing orchestration so Notion can act as the read/write provider.
3. Reuse the shared test-server pattern and retry helpers from `pm::mod.rs`.

## Prereqs

- ~~Bucketed priority model across prompts, ingest, Asana, and Linear~~ — shipped
- Item 08: OAuth-only PM auth
- Item 09: Notion README sync
- Item 10: Notion supporting-doc import

## Constraints

- Notion should not be the thing that forces the bucketed-priority redo; that must already exist.
- Notion task sync should speak the shared meaning but preserve doc-native workflow where possible.
- Keep the adapter thin; docs import and task sync may share a client, but they are different surfaces.

## Done when

- Notion can participate in `pm init`, `pm pull`, and `pm sync`
- The remote schema expresses the shared priority buckets cleanly
- Notion README/docs sync remains a first-class part of the story rather than an afterthought
