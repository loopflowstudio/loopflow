-- The organ cut: the trigger/activation machinery and the agent process
-- ledger left the daemon (webhooks exec `lf`, cron lives in the wave's
-- resident mind, dispatch is placed `lf`), so their tables go. Crons now live in
-- GOAL.md frontmatter.
--
-- runs.activation_log_id must go FIRST: it carries a foreign key into
-- activation_log, and with the parent table gone every statement touching
-- runs fails to prepare. Dropping the column drops the constraint with it
-- (portable: sqlite >= 3.35 via the bundled rusqlite).
ALTER TABLE runs DROP COLUMN activation_log_id;

-- IF EXISTS keeps the drops convergent on every history — including dbs old
-- enough to still carry the pre-rename `stimuli` table.
DROP TABLE IF EXISTS pending_activations;
DROP TABLE IF EXISTS activation_log;
DROP TABLE IF EXISTS triggers;
DROP TABLE IF EXISTS stimuli;
DROP TABLE IF EXISTS agents;
DROP TABLE IF EXISTS wave_crons;
