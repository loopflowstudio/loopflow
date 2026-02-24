# 05: Credential Hygiene

Reduce credential exposure without introducing a complex key-tier system.

## What exists after this

- lfd daemon auth token and user/session auth tokens are stored and handled separately.
- Static-token deployments have a clear token rotation flow.
- Secrets are not persisted accidentally through config writes or surfaced in logs/errors.
- Tokens are carried in headers, never URL/query strings.

## Security boundary for this phase

This phase reduces accidental credential exposure:

- Secrets are less likely to leak through config writes, logs, status, and errors.
- Token roles are less likely to blur across daemon and user/session contexts.
- Static credentials have a supported rotation path.

This phase does not provide:

- A full IAM/key-management system.
- Protection from same-user local process compromise reading locally stored secrets.

## What we learned from phases 01–04

- Session token and static token paths are distinct since Phase 01. Remaining work is ensuring no silent fallback between them.
- Credential file mounts (`executor.credentials.mounts`) are global — all waves sharing a repo get the same credential mounts. Per-wave credential scoping is not in scope here but is worth tracking if multi-tenant needs arise.
- Phase 04 shipped centralized error payload sanitization (redacts tokens, paths, host identifiers from HTTP error responses) and `SafeHttpClient` (strips auth headers on outbound redirect authority changes). Error payload redaction is done; remaining redaction work is logs and status/debug endpoints.
- Phase 04 also added client-side token-leak prevention tests in Python and Swift suites.

## Scope (lightweight by design)

### Token separation

Phase 01 established the session token (`~/.lf/session-token`) as distinct from static tokens. The remaining work:

- Ensure no silent fallback between token types at the middleware level (session token accepted only for `Local` provider, static token only for `Static` provider).
- Verify that client-side token resolution doesn't mix credentials across provider types.

### Rotation

- One supported static-token rotation path:
  - generate new token
  - atomically swap config/secret source
  - reload/restart daemon
  - invalidate old token
- Documented operational runbook.

### Secret-safe persistence and output

- Config write paths preserve `${ENV_VAR}` references instead of resolving and writing secrets.
- Redaction in:
  - logs
  - status/debug endpoints
  - ~~error payloads~~ (done — shipped in Phase 04 via centralized error sanitization)
- File permissions for persisted secret-bearing files remain strict (`0600`).

### Transport hygiene

- Reject auth tokens in query params for lfd APIs.
- Require bearer-token headers for authenticated routes.

## Done when

- Separation between daemon and user/session tokens is enforced in code.
- Rotation path exists, is documented, and has regression coverage.
- Config/log/status paths do not leak secrets. (Error path redaction done in Phase 04.)
- URL/query token submission is rejected on authenticated endpoints.
