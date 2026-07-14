ALTER TABLE task_sessions
ADD COLUMN agent TEXT NOT NULL DEFAULT '';

UPDATE task_sessions
SET agent = provider
WHERE agent = '';
