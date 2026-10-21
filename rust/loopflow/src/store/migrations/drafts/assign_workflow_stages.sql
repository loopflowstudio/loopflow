-- This Machine's sharing settings, on its own local row and never exchanged.
-- Pickup: this Machine lists work addressed to its person. A public name is
-- what teammates read for this Machine; private Machines publish no label.
ALTER TABLE machines ADD COLUMN pickup INTEGER NOT NULL DEFAULT 1 CHECK (pickup IN (0,1));
ALTER TABLE machines ADD COLUMN public_name TEXT;
