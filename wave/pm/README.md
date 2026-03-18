# PM

## Vision

Loopflow syncs with the planning tools teams already use. Asana, Linear, and Notion are all first-class PM providers with full item sync. The next frontier is making Notion the doc-native source that brings README context and supporting docs into the wave instead of flattening everything into tasks/issues. ff64d8ac6 (lf commit: implement)

### Not here

- Jira or other providers beyond Asana, Linear, Notion
- Board/kanban view sync (sections, statuses, columns)
- Bidirectional real-time sync or webhook-driven merge logic

## Strategy

The PM architecture now centers on provider roles, a shared seam, and a single set of file formats:

- `rust/loopflow/src/lfd/pm/mod.rs` owns the provider-agnostic language (`PmProviderKind`, `PmConfig`, `PmItem*`, `PmProvider`, `PmTextUpdate`, `RoadmapItemDocument`), shared retry logic (`RATE_LIMIT_RETRIES`, `retry_after_delay`), and the shared test server (`test_server` module)
- `rust/loopflow/src/lfd/pm/asana.rs`, `rust/loopflow/src/lfd/pm/linear.rs`, and `rust/loopflow/src/lfd/pm/notion.rs` are the concrete transport adapters; all carry `PriorityBucket` through their native vocabularies (Asana custom-field labels, Linear native priorities, Notion select properties)
- `RoadmapItemFrontmatter` uses per-provider ID fields (`asana_id`, `linear_id`, `notion_id`) with `id_for(provider)` and `set_id(provider, id)` dispatch, enabling multi-provider linking without a second frontmatter shape
- `rust/loopflow/src/ops/pm.rs` owns role-aware bootstrap/status/import/sync orchestration (`rw_provider` plus `export_providers`) and writes wave YAML/frontmatter through the shared helpers
- `rust/loopflow/src/ops/export.rs` pushes local roadmap state to every configured provider role and can create missing provider projects
- `WaveExecutor::execute()` already imports from the read/write provider at PR-oriented run start and exports back to configured providers at the end; future work should keep using that lifecycle instead of inventing a second sync path

Prompts, ingest, and provider sync now assume four priority levels (Urgent / High / Medium / Low, files prefixed `1-` through `4-`) and translate that meaning into the native language of each PM tool. Notion item descriptions sync as real pages with full markdown↔blocks conversion (`pm/notion_blocks.rs`), not flattened text. The next steps are wiring ingest to auto-refresh from PM before picking, cleaning up auth to OAuth-only, and extending Notion into doc-native README and supporting-doc sync.

### Invariants

- Provider clients stay thin. They translate API semantics; they do not read config files, mutate wave markdown, or own credential lookup policy.
- `lf op auth` remains the single local credential surface. PM auth should converge on browser-based OAuth rather than a mix of OAuth and API-key setup paths.
- `RoadmapItemDocument` stays the only writer for roadmap frontmatter. PM sync code should use `id_for(provider)` / `set_id(provider, id)` for provider-ID access, not open-coding frontmatter mutations.
- Provider roles stay explicit: one read/write provider drives local state; export providers mirror writes but never become import sources.
- Import is a pull: the read/write PM state wins on conflicts. Export is a push: loopflow only writes back on explicit push events with known local diffs or lifecycle payloads.
- Default day-to-day usage is pull. `lf ops pm pull` rewrites local wave files from PM without consulting `main`; push paths stay explicit and event-scoped.
- Automatic wave-level import/export is now the default lifecycle path for PR-oriented runs. Remaining work should hook into that path rather than inventing extra sync entrypoints.
- Missing config (`asana.workspace`, `asana.default_team`, `linear.team`) should fail with actionable messages at the command boundary, not opaque provider errors.
- `PmTextUpdate` filters rank-only updates at the trait boundary. Providers never see rank changes — rank is a local concern.
- Item-level PR/merge/failure sync must survive `ingest` moving a roadmap item into `scratch/`. Stable item identity belongs on the run, not in a transient file lookup.

## Goals

- Wire ingest auto-refresh from PM so `lf ingest` sees the latest remote state before picking
- Complete item lifecycle comments (PR open, run failure, merge → comment/complete on PM item)
- Move PM auth toward OAuth-only browser-connect flows
- Add Notion README sync and supporting-doc import now that the Notion client is proven
- Add Asana rich-text round-tripping via `html_notes` to match Notion's formatting fidelity

## Risks

- **Within-level choice is unresolved.** The priority model is landed, but multiple items at the same level use filename order as a local fast path. Anything that assumes a deterministic total order will need a follow-up rule.
- **Asana label recognition is brittle.** Priority mapping relies on custom-field option names being semantically recognizable (`Urgent`/`High`/`Medium`/`Low`). Unexpected labels will need follow-up.
- **Item identity is still fragile.** `ingest` moves a roadmap item into `scratch/`, and current runs do not retain a durable link back to that item for later PR/merge comments or completion.
- **Notion body rewrites are destructive.** `update_item` deletes top-level blocks and re-appends the new body, so concurrent edits in Notion and locally can conflict at the page-body level. No merge — last writer wins.
- **Notion read amplification.** `list_items` is inherently N+1 (1 database query + 1 block-children fetch per page). Inherent to Notion's API — no workaround.
- **Credential/config drift is user-facing.** PM flows will feel broken unless missing workspace/team configuration points to the exact knob the user needs to set.
- **Linear `completed_state_id` not cached.** Each `complete_item` call makes two API requests. Acceptable for wave-scale usage but would need caching at higher volumes.

## Metrics

- Import/export round-trip fidelity for title, description, and order: 100%
- Sync latency from merge to remote completion: <30s
- Redundant API calls during steady-state sync: 0
