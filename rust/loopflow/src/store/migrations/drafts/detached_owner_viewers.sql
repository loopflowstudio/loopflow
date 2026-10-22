-- An execution owner's viewers: whether one is attached, and when the last one
-- left. Both stay NULL on owners from builds that never reported viewers.
ALTER TABLE processes ADD COLUMN viewed INTEGER CHECK(viewed IN (0,1));
ALTER TABLE processes ADD COLUMN detached_at INTEGER;
