# Draft migrations

Author a schema change here with `scripts/new_migration.py <name>`. A draft carries
a stable snake_case name and no ordinal; the release cut (`lf release run`) orders
the accumulated drafts and assigns canonical `<major>.<minor>.<ordinal>` ids. See
`../MIGRATIONS.md`. This directory is empty between releases.
