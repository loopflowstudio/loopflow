-- Codex input already includes cached tokens. Other provider receipts keep
-- cache input separate, so normalize their historical lifetime totals here.
UPDATE agent_turns
SET provider_total_input_tokens = provider_input_tokens
    + COALESCE(cache_read_tokens, 0)
    + COALESCE(cache_write_tokens, 0)
WHERE provider_input_tokens IS NOT NULL
  AND (SELECT provider FROM agent_launches WHERE id = launch_id) != 'codex';
