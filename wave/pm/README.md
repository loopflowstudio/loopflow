# PM

## Vision

Loopflow syncs with the PM tools teams already use. Plan in Asana or Linear, execute in loopflow, results flow back. The wave is the unit of work in both systems.

### Not here

- Jira, Notion, or other providers (future waves)
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync (loopflow owns `.md` files, PM tools own planning)

## Strategy

Build everything on the shared PM seam that already exists in `rust/loopflow/src/lfd/pm.rs`: provider kind, wave config, roadmap frontmatter, and provider-agnostic item types. Provider clients, ops commands, ingest hooks, and run-lifecycle sync should extend that seam instead of inventing provider-specific side paths.

### Model

- Wave ↔ project
- Roadmap item ↔ Asana task / Linear issue
- `wave/<name>/<name>.yaml` carries the provider + project in `pm`
- Roadmap item frontmatter carries provider-agnostic `pm_id`
- External IDs stay strings end to end (Asana GID strings, Linear UUID/project IDs)

### Ownership

Provider credentials live in the existing encrypted provider-token store. Reuse that storage and lookup path for PM work, but keep PM UX separate from metered model-provider UX — Asana and Linear are API-key providers, not pay-per-token model providers.

Import is a pull: the PM tool wins on conflicts. Export is a push: loopflow's markdown and filename order become the desired remote state. Avoid bidirectional merge logic.

## Goals

- Asana and Linear clients implement `PmProvider`
- Bootstrap CLI creates/links projects in either direction
- `import-pm` / `export-pm` steps compose into flows
- `ingest` auto-refreshes from tracker when PM is configured
- Run lifecycle events sync completion back to the PM tool

## Risks

- **Asana rich text vs markdown.** Asana descriptions are rich text (HTML-ish), not markdown. Conversion may lose formatting or produce ugly results. Linear is native markdown.
- **Rate limits differ.** Asana: 1500 req/min. Linear: 400 req/min. A wave with 50 items could hit Linear's limit during bulk operations.
- **Ordering semantics differ.** Asana tasks have explicit sort order. Linear issues have priority levels but ordering within a priority is less structured.
- **Credential metadata may need to grow.** PM credentials currently reuse the generic provider-token shape. If later sync needs workspace/team metadata at auth time, extend that model carefully instead of bolting on PM-only storage.
- **Roadmap frontmatter parsing is narrow today.** `RoadmapItemDocument` assumes the repo's current `--- ... ---` frontmatter shape. Import/export work should normalize files through that helper instead of open-coding markdown edits.

## Metrics

- Import/export round-trip fidelity: items created in PM tool appear correctly in `wave/` and vice versa (target: 100% for name, description, order)
- Sync latency: time from PR merge to PM item marked complete (target: <30s)
- API calls per sync operation (target: 1 per item + 1 for project, no redundant calls)
