# 01: Security Hardening

**Finish line:** No credentials visible in Debug output. lfd container runs as non-root. `@unchecked Sendable` uses are audited.

## Scope

**Credential redaction.** `GitHubConfig` derives `Debug` with `token: Option<String>` — the token is printable in debug output. Replace with `SecretString` or implement a manual `Debug` that redacts it.

**Container hardening.** Add a non-root user to `docker/lfd/Dockerfile` (the agent image already has one). Pin `opencode` to a specific version with checksum verification in `install-loopflow.sh`.

**`@unchecked Sendable` audit.** 18 uses across 14 Swift files — services (`AuthService`, `DiscoveryService`, `LocalWaveService`, `VoiceInputService`, `NotificationService`, `CertificatePinningDelegate`, `CredentialSocketServer`, `ConnectionSecretStore`, `TokenProvider`), design (`BrandColors`), and tests. Document why each is safe or fix the underlying concurrency issue.

**`BudgetConfig::default()` trap.** `BudgetConfig` derives `Default`, but `Default::default()` gives zeros while serde deserialization gives the real defaults (50k/30k/20k). Make `Default` produce the real values, or remove the `Default` derive and require explicit construction.

## Already verified safe

**Webhook security.** `github_webhook_handler` returns HTTP 503 when `webhook_secret` is empty. `verify_webhook_signature` rejects empty secrets. HMAC uses constant-time comparison.

**Studio auth caching.** `TokenLedger` implements a 1-hour TTL cache for validated connection tokens.
