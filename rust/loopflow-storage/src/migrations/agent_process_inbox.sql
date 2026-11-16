-- Where the Machine server reaches an idle conversation that has no endpoint:
-- the key file Claude publishes for its cross-session inbox,
-- `<config>/sessions/<pid>.<sha256 of socket>.key`. A path, never the key.
ALTER TABLE processes ADD COLUMN inbox TEXT;
