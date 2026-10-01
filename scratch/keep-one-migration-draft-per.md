# LOO-344: one migration draft per Task, edited in place

Status: implemented, unreviewed. Jack Heart (2026-09-30): "We dont have to support
any clients other than literally this machine."

## Outcome

A Task owns one draft file and rewrites it until landing. No intermediate
schema exists, so nothing tests or protects one.

## Decisions (Claude, 2026-10-01; Jack is not reviewing this Task)

- A draft is `drafts/<name>.sql`. The filename is its identity. The 128-bit
  authoring id and the `-- name:` / `-- id:` headers go away; `-- depends_on:`
  stays because drafts from different Tasks still need an order at the cut.
- `new_migration.py` adapts instead of refusing: when the branch already
  carries a draft that its merge base with main lacks, it prints that file and
  creates nothing.
- Unreleased drafts already on main are editable too. A Task that needs to undo
  one edits it instead of adding a create-then-drop pair.
- Custom `LF_HOME` stores keep exact-schema validation only. The
  `development_migrations` receipt ledger (id/name/checksum per applied draft)
  protected stores that had applied an older draft; those stores are disposable.
- Skills tell agents to test released frontier → finished draft, never the
  steps between.

## Delete — do not maintain

- `-- id:` / `-- name:` headers, `__<id>` filenames, id minting.
- `DraftManifestErrorCategory::{MissingName, NameMismatch, MissingId,
  InvalidId, IdMismatch, DuplicateName}` and their fixture cases.
- `MigrationDraft.{id, checksum, dependencies}`, `DraftMigration.{id, checksum}`.
- `development_migrations` table, `AppliedDevelopmentMigration`,
  `_applied_development_migrations`, `_validate_applied_drafts`,
  `_validate_draft_manifest`, receipt listing in the experimental diagnostic.
- Tests exclusive to the above.

Preserve: dependency ordering, released-name collision, reserved `-- draft:`
marker, canonical batch bytes, promotion refusal for draft-bearing builds,
experimental exact-schema validation and concurrent-initializer rechec## Remaining

- Gate/CI: full Rust suite under draft materialization. Two lib tests
  (`legacy_persisted_json_upgrades_to_typed_stable_tasks`,
  `migration_preserves_planning_identity_and_removes_snapshot_storage`) fail in
  an unmaterialized local tree because they apply canonical migrations and then
  open a store that expects the seven pending drafts; neither path changed here.
- Per-draft boundary tests for already-released drafts in
  `store/migrations.rs` stay: they cover published history, not intermediate
  schemas.

Check: `cargo clippy --all-targets -- -D warnings` clean; `cargo test -p loopflow
--lib -- migration_drafts store::migrations build_info store::sqlite` 166 pass,
the 2 above fail; `flow_tests` 23 pass; migration/architecture pytest 77 pass;
`check_migrations.py` and `check_architecture.py` pass.
