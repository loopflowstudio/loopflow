# 05: Security Hardening

**Finish line:** No credentials visible in Debug output. lfd container runs as non-root. Studio auth has a validation cache.

## Scope

**Credential redaction.** Replace `Option<String>` with `SecretString` for the GitHub token in `GitHubConfig`, or implement a manual `Debug` that redacts it. `GitHubConfig` derives `Debug` — the token is currently printable.

**Container hardening.** Add a non-root user to `docker/lfd/Dockerfile` (the agent image already has one). Pin `opencode` to a specific version with checksum verification in the agent Dockerfile.

**Studio auth caching.** Studio mode makes a network call to the Studio API on every authenticated request. Add a short TTL cache (e.g., 60s) for validated connection tokens to reduce per-request latency and eliminate the DoS amplification vector.

**`@unchecked Sendable` audit.** Review all 6+ uses in Swift Concerto services (`WaveService`, `AuthService`, `DiscoveryService`, `CredentialSocketServer`, `CertificatePinningDelegate`, `LoopflowPalette`). Document why each is safe or fix the underlying concurrency issue.

**`BudgetConfig::default()` trap.** Make `BudgetConfig` and `AutopruneConfig` implement `Default` with the real default values (50k/30k/20k) instead of zeros. Or remove the `Default` derive and require explicit construction.

**Webhook security — verified safe, no work needed.** Research during the daemon integrity sprint confirmed: `github_webhook_handler` returns HTTP 503 when `webhook_secret` is empty (before processing any payload), `verify_webhook_signature` rejects empty secrets as a second layer, and HMAC uses constant-time comparison.
