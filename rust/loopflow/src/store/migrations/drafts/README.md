# Draft migrations

A Task keeps one draft here, `<name>.sql`, and edits it in place until it lands.
`scripts/new_migration.py <name>` creates it, or prints the one the branch already
has. The file is the draft's registration; the release cut (`lf repo release run`)
orders the accumulated drafts and publishes one canonical
`<major>.<minor>.<patch>.<ordinal>_release` batch. Corrections append a batch
within the same unpublished version without changing earlier migration bytes. See `../MIGRATIONS.md`. This directory
is empty between releases.
