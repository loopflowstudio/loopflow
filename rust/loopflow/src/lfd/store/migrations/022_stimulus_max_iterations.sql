-- Add max_iterations column to stimuli for safety valve on looping waves.
ALTER TABLE stimuli ADD COLUMN max_iterations INTEGER;
