# PM

## Vision

Loopflow syncs with the PM tools teams already use. Plan in Asana or Linear, execute in loopflow, results flow back. The wave is the unit of work in both systems.

### Not here

- Jira, Notion, or other providers (future waves)
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync (loopflow owns `.md` files, PM tools own planning)

## Strategy

Build the trait first with two implementations side by side. Asana (REST) and Linear (GraphQL) prove the abstraction works across API styles. Bootstrap CLI makes setup trivial. Steps make sync composable. Ingest integration makes it invisible.

## Goals

- `PmProvider` trait abstracts project/item CRUD across providers
- Asana and Linear clients implement the trait
- Bootstrap CLI creates/links projects in either direction
- `import-pm` / `export-pm` steps compose into flows
- `ingest` auto-refreshes from tracker when PM is configured
- Run lifecycle events sync completion back to the PM tool

## Risks

- **Asana rich text vs markdown.** Asana descriptions are rich text (HTML-ish), not markdown. Conversion may lose formatting or produce ugly results. Linear is native markdown.
- **Rate limits differ.** Asana: 1500 req/min. Linear: 400 req/min. A wave with 50 items could hit Linear's limit during bulk operations.
- **Ordering semantics differ.** Asana tasks have explicit sort order. Linear issues have priority levels but ordering within a priority is less structured.
- **The trait may be too thin.** Provider-specific features (Asana sections, Linear cycles, labels, assignees) don't fit the common interface. Resist expanding the trait — provider-specific features belong in provider-specific code.

## Metrics

- Import/export round-trip fidelity: items created in PM tool appear correctly in `wave/` and vice versa (target: 100% for name, description, order)
- Sync latency: time from PR merge to PM item marked complete (target: <30s)
- API calls per sync operation (target: 1 per item + 1 for project, no redundant calls)
