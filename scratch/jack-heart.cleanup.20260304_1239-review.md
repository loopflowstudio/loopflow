# Gate Review: Trust Hardening and Encryption

## What was implemented

Six hardening changes shipped as one coherent trust pass:

1. **BudgetConfig::default() fix** — manual `Default` impl returns 50k/30k/20k instead of derived zeros. Serde defaults already worked; this fixes the non-serde path.

2. **Container hardening** — lfd Dockerfile adds non-root `lfd` user. `install-loopflow.sh` pins opencode to v1.2.17 with per-arch SHA256 checksums.

3. **`@unchecked Sendable` audit** — every use across 10 production files and 4 test files now has a `// SAFETY:` comment. `BrandColors` (`LoopflowPalette`) changed from `@unchecked Sendable` to proper `Sendable` conformance (immutable value type). Enums in VoiceInputService (`InputMode`, `VADSensitivity`, `VADEvent`) also gained `Sendable`.

4. **Owner identity binding** — `owner_sub_from_jwt()` extracts `sub` claim via base64url decode (no signature verification — studio already validated). Stored on `RegistrationState`, set during `register()`. Test confirms extraction.

5. **Per-field token encryption** — new `token_crypto` module implements AES-256-GCM encryption with platform keychain key storage (macOS Keychain → Linux secret-tool → `~/.lf/provider-token.key` fallback). Migration 027 adds `encrypted BOOLEAN DEFAULT 0`. Both SQLite and Postgres backends encrypt on write, decrypt on read, and migrate plaintext rows on startup.

6. **Credential redaction** — `GitHubConfig.token` uses `SecretString`, matching `AuthConfig`. `expose_secret()` called at use sites (`live_pr.rs`, `waves.rs`).

## Key choices

**AES-256-GCM per-field over SQLCipher.** Only two columns need protection. Whole-DB encryption is a heavy dependency that breaks tooling and is overkill here.

**Platform keychain with file fallback.** macOS Keychain for macOS, `secret-tool` for Linux, file at `~/.lf/provider-token.key` (0o600) for Docker/headless/CI. Key cached in-process via `OnceCell`. The key never lives in the database.

**`encrypted` column for migration.** Default 0 marks legacy rows. Startup migration encrypts in-place within a transaction. Once all rows are encrypted, the column is inert — no separate migration table needed.

**Owner `sub` extraction without JWKS.** Connection tokens (short-lived, single-use, locally validated via TokenLedger) already prevent unauthorized access. JWKS validation would add round-trip complexity for no realistic attack vector.

## How it fits together

`token_crypto` is a standalone module that handles key management and AES-256-GCM operations. Both store backends (`sqlite.rs`, `postgres.rs`) call it symmetrically: `encrypt_token()` on write, `decrypt_if_needed()` on read, `migrate_plaintext_provider_tokens()` on connection open. The `encrypted` boolean column in `provider_tokens` gates whether decryption is needed, making the migration transparent to callers.

`owner_sub_from_jwt()` is a pure function on `registration.rs` — no new state management, just base64url decode of the JWT payload's `sub` claim.

## Risks and bottlenecks

- **Key loss = token loss.** If the keychain entry and fallback file are both lost, encrypted tokens become unrecoverable. No key rotation mechanism yet — acceptable for v1, but worth noting.
- **Keychain prompts on macOS.** First `security add-generic-password` may trigger a Keychain Access dialog in non-headless contexts. The fallback path handles this gracefully.
- **Flaky test.** `wave_rename_renames_branch` fails intermittently due to a timestamp race in branch naming. Pre-existing, not introduced by this branch.

## What's not included

- Studio auth changes (already working, not touched)
- `allowed_users` config (unnecessary — studio gates connection tokens per-user)
- JWKS validation (connection-token architecture makes it redundant)
- Sandbox integration (items 04-05, separate scope)
- Key rotation mechanism (future enhancement)
- Wave trust items deleted from `wave/trust/` — 01, 02, 03 are now completed or descoped
