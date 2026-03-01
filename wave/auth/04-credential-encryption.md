# 04: Credential Encryption at Rest

**Finish line:** API keys and OAuth tokens in lfd's database are encrypted at rest. Reading the SQLite file directly reveals nothing usable.

## Context

Sprint 02 stores API keys in the same `access_token` column as OAuth tokens, unencrypted. OAuth tokens are short-lived and auto-refresh, so exposure is time-bounded. API keys are long-lived — a leaked DB file means a leaked key.

## What to consider

- Encryption key management — where does the decryption key live? macOS Keychain, filesystem, derived from user password?
- Impact on `credential_type` handling — `env_var_for_token` reads plaintext today, needs a decrypt step
- Migration path for existing unencrypted tokens
- Whether OAuth tokens also warrant encryption (lower risk but consistent)
- Performance impact on token refresh loops and executor credential injection

## Risks

- Docker executor's per-provider DB calls (3 per container launch) would each add a decrypt step. Sprint 02 already noted this as a batching opportunity — batching + encryption should land together.
