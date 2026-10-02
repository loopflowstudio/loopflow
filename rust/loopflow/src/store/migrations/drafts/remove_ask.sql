-- depends_on: primary_session_scope

UPDATE agent_sessions SET kind='conversation', primary_scope=NULL,
    skill=CASE WHEN skill='unblock' THEN NULL ELSE skill END
WHERE kind='ask';
