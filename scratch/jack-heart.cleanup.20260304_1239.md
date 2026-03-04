# Trust: Hardening and Encryption

## Problem

Two trust concerns share overlapping code paths — credential handling and token storage. Hardening fixes (redaction, BudgetConfig) are prerequisites for safely debugging encryption issues. Shipping them together means credentials are redacted and encrypted through a single coherent path.

Studio auth (sign-in, discovery, daemon picker, connection tokens) is already built and working. This PR doesn't touch it.

## Scope

**In scope:** credential redaction, BudgetConfig fix, lfd Dockerfile hardening, opencode version pinning, Sendable audit, per-field token encryption.

**Out of scope:** studio auth changes (already working), `allowed_users` (unnecessary — studio gates connection tokens per-user), JWKS validation (connection-token architecture already provides local validation via TokenLedger), sandbox integration (items 04-05).

## Changes

### Credential redaction (Rust)

Replace `token: Option<String>` in `GitHubConfig` with `token: Option<SecretString>` from the `secrecy` crate, matching `AuthConfig` which already uses it. Implement `Debug` manually to print `[REDACTED]`. Update any code that reads `.token` to call `.expose_secret()`.

### BudgetConfig::default() fix (Rust)

Implement `Default` manually to return real values (50k/30k/20k) instead of deriving it. The derived `Default` produces zeros, which silently breaks any code path that doesn't go through serde.

### Container hardening (Docker)

Add non-root user to `docker/lfd/Dockerfile` following the agent image pattern:
```dockerfile
RUN addgroup --system lfd && adduser --system --ingroup lfd lfd
RUN chown -R lfd:lfd /app
USER lfd
```
Pin `opencode` to a specific release version with SHA256 checksum in `install-loopflow.sh`.

### `@unchecked Sendable` audit (Swift)

Add `// SAFETY:` comments to each use explaining why concurrent access is safe. Fix any that aren't.

Categories after investigation:
- **NSObject subclasses** (AuthService, NotificationService, CertificatePinningDelegate): Main-actor isolated, no cross-thread mutation. Document.
- **Keychain/store wrappers** (ConnectionSecretStore, CertificatePinStore, KeychainTokenProvider): Keychain API is thread-safe. Document.
- **Network services** (DiscoveryService, CredentialSocketServer): URLSession is thread-safe; socket server needs review for mutable state.
- **Audio engines** (WhisperKitVoiceInputEngine, AppleDictationVoiceInputEngine): Already documented — third-party types aren't Sendable.
- **WaveService**: Complex generic factory — review closure captures.
- **BrandColors**: Immutable. Remove `@unchecked`, conform properly.
- **Test types**: Acceptable. Add brief comment.

### Owner identity binding (Rust)

Decode the `sub` claim from the registration JWT at startup (base64url decode of the payload — no signature verification needed, studio already validated it during registration). Store as `owner_sub` on `RegistrationState`. Log it so operators can see which user owns the daemon.

When studio issues connection tokens via heartbeat, they're already scoped to the registering user. This just makes lfd aware of its own owner, which sets up for multi-user access control later without building new plumbing.

### Encrypt credentials at rest (Rust)

**Key management.** Platform keychain (macOS Keychain via `security` CLI, Linux `secret-tool` or file-based fallback). The encryption key is a 256-bit AES key stored outside the database. On first run or migration, generate the key and store it in the keychain.

**Encryption scheme.** AES-256-GCM per-field encryption. Each `access_token` and `refresh_token` value gets a unique nonce. Ciphertext stored as base64 in the existing TEXT columns. An `encrypted` boolean column (default false) distinguishes migrated from legacy rows.

**Migration.** On startup, scan `provider_tokens` for rows where `encrypted = false`. Encrypt in-place, set `encrypted = true`. One-time, transparent.

**Read path.** `env_var_for_token()` and any other token consumer calls a `decrypt_token()` helper before use. The helper loads the encryption key once per process lifetime and caches it in memory.

**Schema change:**
```sql
ALTER TABLE provider_tokens ADD COLUMN encrypted BOOLEAN NOT NULL DEFAULT 0;
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| SQLCipher for whole-DB encryption | Simpler — encrypt everything | Heavy dependency, breaks existing tooling that reads the DB, overkill when only 2 columns need protection |
| Derive key from user password | No external key storage | lfd runs as a daemon without interactive login. No password available at startup |
| JWKS validation for studio auth | Defense in depth | Connection tokens (short-lived, single-use, revocable via TokenLedger) already provide local validation without round-trips. Adding JWKS is redundant complexity for no realistic attack vector |

## Key decisions

**SecretString over manual Debug.** Compile-time prevention of accidental exposure, not just debug-output redaction. Matches existing `AuthConfig` pattern.

**Per-field encryption over whole-DB.** Only `access_token` and `refresh_token` need encryption. AES-256-GCM is lightweight and targeted.

**Platform keychain for encryption key.** macOS Keychain on macOS, `secret-tool` on Linux, file-based fallback for Docker/headless. The key never lives in the database or in config files.

**Owner identity, not `allowed_users`.** lfd extracts `sub` from its registration JWT and knows its owner. No config file listing allowed users — studio already gates who gets connection tokens. Extracting `sub` is cheap and sets up for multi-user later.

**`encrypted` column for migration.** Simpler than a separate migration table or version flag. Rows migrate transparently on startup. Once all rows are encrypted, the column is inert.

## Done when

- `cargo test --all` passes with no credential fields visible in Debug output
- `BudgetConfig::default()` returns 50k/30k/20k
- lfd Docker container runs as non-root user `lfd`
- `opencode` install pinned to specific version with checksum
- Every `@unchecked Sendable` has a `// SAFETY:` comment or is fixed
- `access_token` and `refresh_token` encrypted in SQLite — reading the file reveals only base64 ciphertext
- Encryption key stored in platform keychain, never in the database
- Migration encrypts existing plaintext tokens on startup
- lfd logs `owner_sub` extracted from registration JWT at startup
- `swift test --package-path swift` passes
