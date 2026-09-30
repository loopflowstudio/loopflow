-- draft: provider_account_credential_identity
ALTER TABLE provider_accounts ADD COLUMN observed_credential_digest TEXT;
