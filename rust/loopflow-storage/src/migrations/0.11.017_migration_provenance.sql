-- Record the immutable migration bytes and the build that applied them.
ALTER TABLE schema_migrations ADD COLUMN checksum TEXT;
ALTER TABLE schema_migrations ADD COLUMN parent_history TEXT;
ALTER TABLE schema_migrations ADD COLUMN build_provenance TEXT;
ALTER TABLE schema_migrations ADD COLUMN source_identity TEXT;
ALTER TABLE schema_migrations ADD COLUMN source_revision TEXT;
ALTER TABLE schema_migrations ADD COLUMN package_version TEXT;
