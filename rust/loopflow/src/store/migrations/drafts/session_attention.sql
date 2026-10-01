-- Keep the passive Session inventory bounded to indexed lifecycle scalars.
CREATE INDEX session_driver_exit ON session_events(
    session_id, receipt_key,
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.outcome') END
) WHERE kind='observed';
CREATE INDEX session_turn_attention ON session_events(
    session_id, provider_thread, provider_turn,
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.status') END
) WHERE kind='completed';
