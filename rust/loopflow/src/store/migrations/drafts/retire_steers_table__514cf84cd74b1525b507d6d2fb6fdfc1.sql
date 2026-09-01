-- name: retire_steers_table
-- id: 514cf84cd74b1525b507d6d2fb6fdfc1
-- depends_on: stable_work_state

-- Steers are durable Work comments now (`TaskEventKind::Steer` /
-- `ProjectEventKind::Steer`, read via the event streams). No code reads or writes
-- the `steers` table; `stable_work_state` still rebuilds it only because it was
-- authored when steers were a table. Retire it.
DROP TABLE IF EXISTS steers;
