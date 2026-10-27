-- Whether a Task's Auto-Ship approval may land its PR. Only someone handing the
-- Task off with `lf ship` grants it; a route into `end` approves running that
-- route, not merging. Approvals recorded before this column are uncertain, so
-- they keep no landing authority.
ALTER TABLE tasks ADD COLUMN ship_lands INTEGER NOT NULL DEFAULT 0;
