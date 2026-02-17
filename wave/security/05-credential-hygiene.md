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

## Scope (lightweight by design)

### Token separation

- Distinct credential slots for:
  - daemon/static service token
  - user/session token (Concerto/CLI auth)
- No silent fallback between token types.

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
  - error payloads
- File permissions for persisted secret-bearing files remain strict (`0600`).

### Transport hygiene

- Reject auth tokens in query params for lfd APIs.
- Require bearer-token headers for authenticated routes.

## Done when

- Separation between daemon and user/session tokens is enforced in code.
- Rotation path exists, is documented, and has regression coverage.
- Config/log/status/error paths do not leak secrets.
- URL/query token submission is rejected on authenticated endpoints.
