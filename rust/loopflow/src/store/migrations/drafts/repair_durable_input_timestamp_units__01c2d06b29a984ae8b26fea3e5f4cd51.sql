-- name: repair_durable_input_timestamp_units
-- id: 01c2d06b29a984ae8b26fea3e5f4cd51
-- depends_on: durable_input_spine

-- `0.11.031_durable_input_spine` copied legacy nanosecond timestamps into
-- columns read as Unix seconds. Preserve every row and normalize only values
-- that cannot already be represented as an OffsetDateTime Unix second.
UPDATE epoch_revisions
SET created_at = created_at / 1000000000
WHERE created_at < -377705116800 OR created_at > 253402300799;

UPDATE steers
SET issued_at = issued_at / 1000000000
WHERE issued_at < -377705116800 OR issued_at > 253402300799;

UPDATE tool_responses
SET responded_at = responded_at / 1000000000
WHERE responded_at < -377705116800 OR responded_at > 253402300799;
