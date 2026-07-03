-- Reintroduce wave ancestry on the durable model. The chord tree was folded
-- into `waves` via `parent_wave_id` in migration 011, then dropped again in 013
-- (and the standalone chord tables in 028). The Wave/Run/Session reduction left
-- no ancestry column at all, so `WaveAgentTree.child_waves` could never be
-- populated.
--
-- A chord is simply a wave that has children — there is no `wave_type` or
-- `position` column. Chord-ness is derived from `children_of(id)` being
-- non-empty; siblings order by `created_at`.
--
-- Nullable (a root wave has no parent) with no default, so ADD COLUMN with a
-- self-referential REFERENCES clause is legal in SQLite. ON DELETE CASCADE
-- mirrors the original 011 semantics: deleting a chord deletes its children.
ALTER TABLE waves ADD COLUMN parent_wave_id TEXT
    REFERENCES waves(id) ON DELETE CASCADE;
CREATE INDEX idx_waves_parent ON waves(parent_wave_id);
