# v0.12.3

v0.12.3 is a focused repair release for Homes carrying Task gate proposals from the Session era. It upgrades that legacy JSON before the current typed store reads it, and reports incompatible persisted data completely without leaving a partial migration behind. Shell installs also enter through Loopflow's guarded promotion boundary rather than replacing the executable around it.

## Existing Task history upgrades cleanly

Older Task records stored gate decisions as terminal `status` values; the current model expects a `done` boolean. This release repairs those records at the store boundary without discarding the context around the decision (#1114).

- `completed` becomes `done: true`; `waiting`, `blocked`, `failed`, and `abandoned` become `done: false`.
- Existing `done` values are left alone, while unrelated JSON fields are preserved.
- The typed upgrade test covers legacy writeback and gate-proposal shapes together, proving that the migrated store opens through the current Task model.
- Persisted-JSON validation now collects every incompatible row across all registered columns before reporting failure. Any failure rolls back the complete migration transaction rather than leaving a partially advanced store.

## Downloads use the same guarded activation as releases

The shell installer now asks the downloaded candidate to promote itself. That keeps binary activation, store compatibility checks, the live-Run fence, and migration authority inside the operation that owns them instead of copying a new executable directly into place (#1114).

- Promotion refuses incompatible stores or active-Run evidence without changing the CLI target.
- Accepted candidates are retained as immutable, content-addressed bytes and atomically become the global `lf`.
- The installed candidate owns any pending store migration, so a shell install follows the same release boundary as other supported installation paths.

## Operational notes

**Use the release shell installer or a supported promotion command while no Runs are active.** The `0.12.3.001_release.sql` batch performs the Task gate-proposal repair. As with other release-scoped migrations, the store advances under the exclusive promotion lock and a failed semantic check leaves the transaction unapplied.

**Migration errors are intentionally broader.** If stored JSON is incompatible with current DTOs, the error now names every affected table, column, and row found in the validation pass. The store remains unchanged; investigate every listed row before retrying rather than treating the first one as the only failure.

## Small changes

- CI can materialize post-release migration drafts under the next disposable patch namespace, so repair drafts added immediately after a tag are exercised before the following release cut.
- Migration guidance now treats persisted JSON shape changes as schema changes: they require an ordinal-free repair draft and a typed upgrade test seeded with the previous shape.
