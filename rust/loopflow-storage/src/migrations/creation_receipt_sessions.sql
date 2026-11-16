-- A new Task's conversation finds the creation it finishes setting up.
CREATE INDEX creation_receipts_admitted_session
    ON creation_receipts(session_id) WHERE milestone = 'admitted';
