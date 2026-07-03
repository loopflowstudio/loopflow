-- Wave spend budget: a hard spend ceiling plus the running accrued total.
-- spend_cap is a JSON object {"rate": cents, "per_iteration": cents} or NULL
-- (uncapped). spent is cumulative agent cost in cents.
ALTER TABLE waves ADD COLUMN spend_cap TEXT;
ALTER TABLE waves ADD COLUMN spent BIGINT NOT NULL DEFAULT 0;
