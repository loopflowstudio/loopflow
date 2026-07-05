ALTER TABLE run_token_usage ADD COLUMN repo TEXT;

CREATE INDEX IF NOT EXISTS idx_run_token_usage_repo_provider
ON run_token_usage(repo, provider);
