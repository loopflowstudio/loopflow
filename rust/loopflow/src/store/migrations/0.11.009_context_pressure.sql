-- Preserve provider-normalized lifetime input and the peak request that came
-- closest to its model's context window.
ALTER TABLE agent_turns ADD COLUMN provider_total_input_tokens INTEGER;
ALTER TABLE agent_turns ADD COLUMN peak_input_tokens INTEGER;
ALTER TABLE agent_turns ADD COLUMN context_window_tokens INTEGER;

-- Existing receipts already distinguish Claude's uncached and cached input.
-- OpenAI-compatible input totals include cached tokens, so adding that column
-- there would double-count historical work.
UPDATE agent_turns
SET provider_total_input_tokens = CASE
    WHEN (SELECT provider FROM agent_launches WHERE id = launch_id) = 'claude'
        THEN provider_input_tokens
            + COALESCE(cache_read_tokens, 0)
            + COALESCE(cache_write_tokens, 0)
    ELSE provider_input_tokens
END
WHERE provider_input_tokens IS NOT NULL;
