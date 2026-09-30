-- draft: provider_account_identity
ALTER TABLE provider_accounts ADD COLUMN observed_email TEXT;
ALTER TABLE provider_accounts ADD COLUMN observed_subject TEXT;
ALTER TABLE provider_accounts ADD COLUMN observed_plan TEXT;
