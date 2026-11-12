-- An opener that launched another harness than the Session's recorded one left
-- the row on the old provider while its provider thread belongs to the new one.
-- Move each such Session to the only provider that recorded its thread; the
-- old provider's model does not carry over.
UPDATE agent_sessions SET provider=pin.provider, model=NULL
FROM (SELECT provider_session_id, MIN(provider) AS provider FROM provider_session_accounts
    GROUP BY provider_session_id HAVING COUNT(*)=1) AS pin
WHERE agent_sessions.provider_thread=pin.provider_session_id
    AND agent_sessions.provider IS NOT pin.provider;
