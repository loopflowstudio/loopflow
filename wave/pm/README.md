# PM

## Vision

Loopflow syncs with the planning tools teams already use. Near-term, that means proving a better roadmap model with Asana and Linear. After that, Notion can become the doc-native source that brings README context and supporting docs into the wave instead of flattening everything into tasks/issues.

### Not here

- Jira or other providers beyond Asana, Linear, Notion
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync or webhook-driven merge logic
- Exact total ordering across every provider; the shared model is moving toward priority buckets first

## Strategy

The PM architecture still centers on provider roles, a shared seam, and a single set of file formats:

- `rust/loopflow/src/lfd/pm/mod.rs` owns the provider-agnostic language (`PmProviderKind`, `PmConfig`, `PmItem*`, `PmProvider`, `PmTextUpdate`, `RoadmapItemDocument`), shared retry logic (`RATE_LIMIT_RETRIES`, `retry_after_delay`), and the shared test server (`test_server` module)
- `rust/loopflow/src/lfd/pm/asana.rs` and `rust/loopflow/src/lfd/pm/linear.rs` are the concrete transport adapters; the next redo should make them prove the new bucketed-priority model before Notion joins the same seam
- `RoadmapItemFrontmatter` remains the place for per-provider item IDs via `id_for(provider)` / `set_id(provider, id)`
- `rust/loopflow/src/ops/pm.rs` remains the orchestration layer for `pm_init`, `pm_pull`, `pm_status`, `pm_import`, and `pm_sync`
- `WaveExecutor::execute()` already imports from the read/write provider at PR-oriented run start and exports back to configured providers at the end; future work should keep using that lifecycle instead of inventing a second sync path

The next design step is to stop treating roadmap planning as a single exact queue. Prompts, ingest, and provider sync should all assume four semantic priority buckets and then translate that meaning into the native language of each PM tool.

### Invariants

- Provider clients stay thin. They translate API semantics; they do not read config files, mutate wave markdown, or own credential lookup policy.
- `lf ops auth` remains the single local credential surface. PM auth should converge on browser-based OAuth rather than a mix of OAuth and API-key setup paths.
- `RoadmapItemDocument` stays the only writer for roadmap frontmatter. PM sync code should use `id_for(provider)` / `set_id(provider, id)` for provider-ID access, not open-coding frontmatter mutations.
- Provider roles stay explicit: one read/write provider drives local state; export providers mirror writes but never become import sources.
- Import is a pull: the read/write PM state wins on conflicts. Export is a push: loopflow only writes back on explicit push events with known local diffs or lifecycle payloads.
- The shared roadmap model should be semantic first: broken/unblock-now, clear next step, committed later, speculative. Providers should speak their native vocabulary where possible.
- Default day-to-day usage is pull. `lf ops pm pull` rewrites local wave files from PM without consulting `main`; push paths stay explicit and event-scoped.

## Goals

- Redo roadmap planning around four priority buckets instead of exact numeric ordering
- Update prompt guidance and `ingest` so waves naturally write and consume bucketed priorities
- Make the new model work cleanly with Asana and Linear before touching Notion integration
- Move PM auth toward OAuth-only browser-connect flows
- Add Notion README sync and supporting-doc import after the shared model is proven
- Add Notion task parity only after the prereqs above land

## Risks

- **The current queue model leaks everywhere.** Built-in prompts, docs, ingest, tests, and PM sync all still talk like roadmap items are `01-*`, `02-*`, etc. The redo will touch more surfaces than the data model alone.
- **Within-bucket choice is unresolved.** The immediate goal is binning, not exact ordering. Multiple `P1` items are fine for now, but anything that assumes a deterministic total order will need a follow-up rule.
- **Provider vocabulary differs.** Asana likely wants a custom field, Linear wants native `Urgent/High/Medium/Low`, and Notion will likely want a select property. The shared meaning is portable; the labels are not.
- **Notion block model complexity remains real.** README sync looks high value, but page/block round-tripping is structurally more complex than Asana/Linear task sync.
- **Credential drift is user-facing.** PM flows will still feel broken until the auth cleanup removes mixed setup paths and points users at the right browser-based connect flow.

## Metrics

- Import/export round-trip fidelity for title, description, and priority bucket: 100%
- Sync latency from merge to remote completion: <30s
- Redundant API calls during steady-state sync: 0
