-- draft: index_captured_event
-- Deleting a session_events row makes SQLite find the rows whose
-- captured_event names it; without this, each delete scanned every row's
-- overflow pages under the write lock.
CREATE INDEX session_events_captured_event ON session_events(captured_event)
    WHERE captured_event IS NOT NULL;
