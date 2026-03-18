# PM

## Vision

Loopflow syncs with the PM tools teams already use. Plan in Asana or Linear, execute in loopflow, and let progress flow back without hand-editing wave files.

### Not here

- Jira or other providers beyond Asana, Linear, Notion
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync or webhook-driven merge logic

## Strategy

The PM architecture centers on a shared seam and a single set of file formats:

- `rust/loopflow/src/lfd/pm/mod.rs` owns the provider-agnostic language (`PmProviderKind`, `PmConfig`, `PmItem*`, `PmProvider`, `RoadmapItemDocument`)
- `rust/loopflow/src/lfd/pm/asana.rs` and `rust/loopflow/src/lfd/pm/linear.rs` are the concrete transport adapters
- `lf ops auth ...`, `lfq auth ...`, provider-token storage, and HTTP auth routes handle Asana OAuth and Linear API-key flows
- `rust/loopflow/src/ops/export.rs` is the starting point for mechanical sync: it dispatches to Asana or Linear based on wave config, can create a missing project, and writes provider IDs (`asana_id` / `linear_id`) back through the shared helpers

Future items should deepen that path instead of creating a second one.

### Invariants

- Provider clients stay thin. They translate API semantics; they do not read config files, mutate wave markdown, or own credential lookup policy.
- `lf ops auth` remains the single local credential surface. Future PM commands should consume stored credentials, not invent provider-specific auth side paths.
- `RoadmapItemDocument` stays the only writer for roadmap frontmatter. PM sync code should normalize file edits through it instead of open-coding markdown mutations.
- Import is a pull: the PM tool wins on conflicts. Export is a push: loopflow's markdown and filename order become the desired remote state.
- Missing config (`asana.workspace`, `asana.default_team`, `linear.team`) should fail with actionable messages at the command boundary, not opaque provider errors.

## Goals

- Bootstrap/link/status commands create or connect Asana and Linear projects without manual YAML or frontmatter edits
- Import/export become provider-aware mechanical ops with built-in steps and a `pm-sync` flow
- `ingest` refreshes from PM before picking the next item when a wave is linked
- Run lifecycle events comment on and complete PM items best-effort after PR activity and merge

## Risks

- **Asana rich text vs markdown.** Import/export still needs a crisp normalization story so descriptions do not thrash on every sync.
- **Ordering semantics differ.** Asana needs relative move operations; Linear may need a documented limitation or a separate ordering strategy.
- **Notion block model complexity.** Notion's rich content model is far more structured than Asana/Linear descriptions. The first pass intentionally keeps it simple (paragraph blocks), but round-trip fidelity will need more work if users start editing descriptions in Notion.
- **Lifecycle sync depends on reliable lookup.** Run → wave → roadmap item → provider ID (`asana_id` / `linear_id`) must resolve cleanly, and failures must stay non-blocking.
- **Credential/config drift is user-facing.** PM flows will feel broken unless missing workspace/team configuration points to the exact knob the user needs to set.

## Metrics

- Import/export round-trip fidelity for title, description, and order: 100%
- Sync latency from merge to remote completion: <30s
- Redundant API calls during steady-state sync: 0
