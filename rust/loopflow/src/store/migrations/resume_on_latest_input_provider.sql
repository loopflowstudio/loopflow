-- An opener that launched another harness than the Session's recorded one left
-- the row on the old provider while its provider thread belongs to the new one.
-- Move each such Session to the harness its current input's manifest recorded.
UPDATE agent_sessions SET provider=input.harness, model=input.model
FROM (SELECT s.id,
        CASE WHEN json_valid(m.payload) THEN json_extract(m.payload,'$.evidence.harness') END AS harness,
        CASE WHEN json_valid(m.payload) THEN json_extract(m.payload,'$.evidence.model') END AS model
    FROM agent_sessions s
    JOIN session_events c ON c.seq=s.current_capture
    JOIN session_events m INDEXED BY session_observation_receipt
        ON m.session_id=s.id AND m.kind='observed' AND m.receipt_key=c.receipt_key||':manifest.json'
    WHERE s.provider_thread IS NOT NULL) AS input
WHERE agent_sessions.id=input.id AND input.harness IS NOT NULL
    AND agent_sessions.provider IS NOT input.harness;
-- An account that recorded the thread names the provider that ran it, even
-- where the manifest was observed before the opener rewrote its harness. The
-- old provider's model does not carry over.
UPDATE agent_sessions SET provider=pin.provider, model=NULL
FROM (SELECT provider_session_id, MIN(provider) AS provider FROM provider_session_accounts
    GROUP BY provider_session_id HAVING COUNT(*)=1) AS pin
WHERE agent_sessions.provider_thread=pin.provider_session_id
    AND agent_sessions.provider IS NOT pin.provider;
