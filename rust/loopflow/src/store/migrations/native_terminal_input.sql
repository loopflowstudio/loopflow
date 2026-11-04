-- The terminal owner retains editor input that has no acknowledged submission.
-- NULL means this owner has received no input; provider replacement does not
-- transfer an editor to a different execution owner.
ALTER TABLE processes ADD COLUMN native_input TEXT;
