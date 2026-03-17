# PM

## Vision

Loopflow syncs with the PM tools teams already use. Plan in Asana or Linear, execute in loopflow, results flow back. The wave is the unit of work in both systems.

### Not here

- Jira, Notion, or other providers (future waves)
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync (loopflow owns `.md` files, PM tools own planning)

## Strategy

The shared PM seam lives in `rust/loopflow/src/lfd/pm/mod.rs`: `PmProviderKind`, `PmConfig`, `PmItem`/`PmItemCreate`/`PmItemUpdate`, `PmProvider` trait (6 async methods), `RoadmapItemDocument` with frontmatter parse/render. Asana is now the reference implementation in `rust/loopflow/src/lfd/pm/asana.rs`. Provider clients, ops commands, ingest hooks, and run-lifecycle sync should extend this seam instead of inventing provider-specific side paths.

Keep the layers narrow:

- Provider clients are pure HTTP adapters constructed from stored token + config. They do not own token-store lookups or wave-file mutations.
- Higher-level ops commands resolve credentials, read wave config, and write roadmap files through `RoadmapItemDocument`.
- New provider code should live beside Asana (`pm/linear.rs`, future `pm/<provider>.rs`) so shared types stay isolated from transport code.

### Model

- Wave ↔ project
- Roadmap item ↔ Asana task / Linear issue
- `wave/<name>/<name>.yaml` carries the provider + project in `pm:` block (parsed as `PmConfig`), plus optional per-wave `team` override when project bootstrap needs a specific team
- Roadmap item frontmatter carries provider-agnostic `pm_id` (parsed via `RoadmapItemDocument`)
- External IDs stay strings end to end (Asana GID strings, Linear UUID/project IDs)

### Ownership

Provider credentials live in the existing encrypted provider-token store (`Provider::Asana`, `Provider::Linear`). PM API-key UX is separate from metered model-provider UX — Asana and Linear don't show pay-per-token billing copy in CLI status/output.

Global config (`engine::config`) carries `AsanaConfig` (workspace, default_team) and `LinearConfig` (team) for project creation paths.

Import is a pull: the PM tool wins on conflicts. Export is a push: loopflow's markdown and filename order become the desired remote state. Avoid bidirectional merge logic.

## Goals

- Linear client implements `PmProvider`, matching the Asana seam and test coverage
- Bootstrap CLI creates/links projects in either direction
- `import-pm` / `export-pm` steps compose into flows
- `ingest` auto-refreshes from tracker when PM is configured
- Run lifecycle events sync completion back to the PM tool

## Risks

- **Asana rich text vs markdown.** Asana descriptions are rich text (HTML-ish), not markdown. Conversion may lose formatting or produce ugly results. Linear is native markdown.
- **Rate limits differ.** Asana: 1500 req/min. Linear: 400 req/min. A wave with 50 items could hit Linear's limit during bulk operations.
- **Ordering semantics differ.** Asana has explicit task order but no numeric rank field; exporting filename order will require relative move operations (`insert_before` / `insert_after`) instead of pretending a rank update exists. Linear issues have priority levels but ordering within a priority is less structured.
- **Project bootstrap is config-sensitive.** Asana project creation depends on `asana.workspace` (and sometimes `default_team`); Linear creation depends on `linear.team`. Bootstrap commands need crisp failures and status output when those values are missing.
- **Credential metadata may need to grow.** PM credentials currently reuse the generic provider-token shape. If later sync needs workspace/team metadata at auth time, extend that model carefully instead of bolting on PM-only storage.
- **Roadmap frontmatter parsing is narrow today.** `RoadmapItemDocument` assumes the repo's current `--- ... ---` frontmatter shape. Import/export work should normalize files through that helper instead of open-coding markdown edits.

## Metrics

- Import/export round-trip fidelity: items created in PM tool appear correctly in `wave/` and vice versa (target: 100% for name, description, order)
- Sync latency: time from PR merge to PM item marked complete (target: <30s)
- API calls per sync operation (target: 1 per item + 1 for project, no redundant calls)
