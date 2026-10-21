-- This Machine's sharing settings, on its own local row and never exchanged.
-- Pickup: this Machine lists work addressed to its person. A public name is
-- what teammates read for this Machine; private Machines publish no label.
ALTER TABLE machines ADD COLUMN pickup INTEGER NOT NULL DEFAULT 1 CHECK (pickup IN (0,1));
ALTER TABLE machines ADD COLUMN public_name TEXT;

-- People fetched from `refs/loopflow/people/*`; a cache of what Git holds.
CREATE TABLE people (
    key TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    accounts TEXT NOT NULL CHECK (json_valid(accounts)),
    updated_at INTEGER NOT NULL
);
