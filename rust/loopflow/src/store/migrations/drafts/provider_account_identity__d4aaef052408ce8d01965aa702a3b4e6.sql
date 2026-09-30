-- name: provider_account_identity
-- id: d4aaef052408ce8d01965aa702a3b4e6
-- depends_on: 

ALTER TABLE provider_accounts ADD COLUMN observed_email TEXT;
ALTER TABLE provider_accounts ADD COLUMN observed_subject TEXT;
ALTER TABLE provider_accounts ADD COLUMN observed_plan TEXT;
