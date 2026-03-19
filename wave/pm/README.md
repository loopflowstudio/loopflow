# PM

## Vision

Loopflow syncs with the PM tools teams already use. Plan in Asana or Linear, execute in loopflow, and let progress flow back without hand-editing wave files.

### Not here

- Jira or other providers beyond Asana, Linear, Notion
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync or webhook-driven merge logic

## Strategy

The PM architecture now centers on provider roles, a shared seam, and a single set of file formats:

- `rust/loopflow/src/lfd/pm/mod.rs` owns the provider-agnostic language (`PmProviderKind`, `PmConfig`, `PmItem*`, `PmProvider`, `PmTextUpdate`, `RoadmapItemDocument`), shared retry logic (`RATE_LIMIT_RETRIES`, `retry_after_delay`), and the shared test server (`test_server` module)
- `rust/loopflow/src/lfd/pm/asana.rs` and `rust/loopflow/src/lfd/pm/linear.rs` are the concrete transport adapters — both implement the full `PmProvider` trait with rate-limit retry and noop-update filtering via `PmTextUpdate`
- `RoadmapItemFrontmatter` uses per-provider ID fields (`asana_id`, `linear_id`) with `id_for(provider)` and `set_id(provider, id)` dispatch, enabling multi-provider linking without a second frontmatter shape
- `lf ops auth ...`, `lfq auth ...`, provider-token storage, and HTTP auth routes handle Asana OAuth and Linear API-key flows
- `rust/loopflow/src/ops/pm.rs` owns all PM orchestration: `pm_init` (bootstrap), `pm_pull` (remote-wins refresh), `pm_status` (sync state), `pm_import` (wave creation from PM), and `pm_sync` (bidirectional sync). Writes wave YAML/frontmatter through the shared helpers
- `WaveExecutor::execute()` already imports from the read/write provider at the start of PR-oriented runs and exports back to the configured providers when those runs finish

Future items should deepen that path instead of creating a second one.

### Invariants

- Provider clients stay thin. They translate API semantics; they do not read config files, mutate wave markdown, or own credential lookup policy.
- `lf ops auth` remains the single local credential surface. Future PM commands should consume stored credentials, not invent provider-specific auth side paths.
- `RoadmapItemDocument` stays the only writer for roadmap frontmatter. PM sync code should use `id_for(provider)` / `set_id(provider, id)` for provider-ID access, not open-coding frontmatter mutations.
- Provider roles stay explicit: one read/write provider drives local state; export providers mirror writes but never become import sources.
- Import is a pull: the read/write PM state wins on conflicts. Export is a push: loopflow only writes back on explicit push events with known local diffs or lifecycle payloads.
- Automatic wave-level import/export is now the default lifecycle path for PR-oriented runs. Remaining work should hook into that path rather than inventing extra sync entrypoints.
- Missing config (`asana.workspace`, `asana.default_team`, `linear.team`) should fail with actionable messages at the command boundary, not opaque provider errors.
- `PmTextUpdate` filters rank-only updates at the trait boundary. Providers never see rank changes — rank is a local concern.
- Item-level PR/merge/failure sync must survive `ingest` moving a roadmap item into `scratch/`. Stable item identity belongs on the run, not in a transient file lookup.
- Default day-to-day usage is pull. `lf ops pm pull` rewrites local wave files from PM without consulting `main`; push paths stay explicit and event-scoped.

## Goals

- Thin step wrappers and a `pm-sync` flow expose ops commands (`pm_pull`, `pm_init`, `pm_status`) to normal flows
- `ingest` refreshes from PM before picking the next item when a wave is linked
- Runs retain stable roadmap-item identity so PR open/failure/merge can comment on or complete the specific linked PM item
- Notion can join the provider-role model without bypassing the shared `PmProvider` seam or frontmatter helpers

## Risks

- **Asana rich text vs markdown.** Import/export still needs a crisp normalization story so descriptions do not thrash on every sync.
- **Ordering semantics differ.** Asana needs relative move operations; Linear may need a documented limitation or a separate ordering strategy.
- **Item identity is still fragile.** `ingest` moves a roadmap item into `scratch/`, and current runs do not retain a durable link back to that item for later PR/merge comments or completion.
- **Repo-default export providers may be too broad.** If some waves need to opt out of mirrored exports, add an explicit per-wave override instead of special-casing execution.
- **Live provider round-trips still need manual verification.** Automated tests cover the Rust behavior, but real Linear/Asana credentials and hosted projects are still the only way to prove the full sync path.
- **Notion block model complexity.** Notion's rich content model is far more structured than Asana/Linear descriptions. The first pass intentionally keeps it simple (paragraph blocks), but round-trip fidelity will need more work if users start editing descriptions in Notion.
- **Credential/config drift is user-facing.** PM flows will feel broken unless missing workspace/team configuration points to the exact knob the user needs to set. Error messages for `pm init`/`pm pull` are better than before but misconfigured `linear.team` or `asana.workspace` values are still a setup footgun — the failure mode is an opaque API error rather than a "run `lf ops auth configure linear` first" pointer.
- **Linear `completed_state_id` not cached.** Each `complete_item` call makes two API requests. Acceptable for wave-scale usage but would need caching at higher volumes.
- **Linear team auto-creation.** `resolve_team_id` creates a "Loopflow" team if none exists. Could surprise users who don't expect team creation.

## Metrics

- Import/export round-trip fidelity for title, description, and order: 100%
- Sync latency from merge to remote completion: <30s
- Redundant API calls during steady-state sync: 0
