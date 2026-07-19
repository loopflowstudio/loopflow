# Draft migrations

Author a schema change here with `scripts/new_migration.py <name>`. A draft carries
a stable snake_case name, an immutable authoring id (a 128-bit token, 32 hex chars),
and no ordinal, living at
`<name>__<id>.sql`. The file itself is the draft's registration — there is nothing to
paste into `migrations.rs`. The release cut (`lf release run`) orders the accumulated
drafts and publishes one canonical `<major>.<minor>.<patch>.001_release` batch. Because two branches
authoring the same name mint different 128-bit ids, they never collide or share an
edit. See
`../MIGRATIONS.md`. This directory is empty between releases.
