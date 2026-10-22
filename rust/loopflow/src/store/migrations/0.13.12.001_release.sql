-- draft: process_terminal
-- Terminal placement belongs to the attached lf process, never a client-file lifecycle.
ALTER TABLE processes ADD COLUMN terminal_id TEXT;
