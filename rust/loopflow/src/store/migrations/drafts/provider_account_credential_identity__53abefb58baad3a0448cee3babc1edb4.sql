-- name: provider_account_credential_identity
-- id: 53abefb58baad3a0448cee3babc1edb4
-- depends_on: provider_account_identity

ALTER TABLE provider_accounts ADD COLUMN observed_credential_digest TEXT;
