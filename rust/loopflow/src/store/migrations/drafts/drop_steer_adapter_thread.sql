-- The Session's agent_sessions.provider_thread is the one selected native
-- conversation; context callbacks follow it, so the adapter keeps no copy.
ALTER TABLE steer_adapters DROP COLUMN provider_thread;
