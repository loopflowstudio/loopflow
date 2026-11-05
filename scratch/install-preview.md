# Installation preview must not write preferences

Jack Heart requested shipping and migration on November 4, 2026. PR #1805 merged
as a5000ef889. Its green package failed normal installation on Jack's Mac:
the CLI's lifecycle preview migrated a copied database whose repository paths
still pointed at live checkouts. Settings cutover created Loopflow's private
config. The app helper's second preview then rejected that generated file as a
precedence conflict. The installed CLI and live database remain unchanged.

Keep one migration engine with an explicit preview purpose. Lifecycle preview
applies SQL and validates migrated data in its snapshot, but does not perform
the settings-file cutover. Real promotion retains collision checking, file
locks, recovery journal and SQL transaction. No released SQL changes.

Delete the preview's use of the ordinary file-writing migration entry. Prove
that two consecutive lifecycle previews leave live global/private files and
the source database untouched, and that the subsequent real migration exports
the same declarations successfully. Existing collision tests must still pass.

Restore only the private file this failed preview generated, after checking its
exact contents against the old DB and saving a copy. Then ship the correction
and retry the normal installer before applying the prepared repo migrations.
The current public package cannot safely complete this cutover; do not bypass
the paired CLI/app installer or run a branch build against the live database.

Limit recovery remains separate work in LOO-714. Infrastructure memory exceeds
its token target by 89 tokens; this implementation has no authority to curate it.

Verification: the repeated-preview regression and 15 related installation/settings
migration tests passed; format, all-target Clippy, architecture coverage and test
inventory passed. Public installation remains blocked until this correction is
merged, packaged and installed through the normal path.
