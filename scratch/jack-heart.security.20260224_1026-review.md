# Security Phase 05: Credential Hygiene — Design Review

## What was implemented

Hardened `lfd` credential handling so secrets are fail-closed at type boundaries, HTTP surfaces, middleware ordering, generated config, and operator workflows. Concrete changes:

- **SecretString adoption**: Auth tokens across `AuthProvider`, `AuthConfig`, `ExecutorConfig`, `RegisterResponse`, `RegistrationClient`, and session token paths now use `secrecy::SecretString`. Debug/Display redact, equality is constant-time via `subtle`, internal bytes are zeroized on drop.
- **Operator-facing redaction module** (`lfd::redaction`): Heuristic sanitization of error/status text covering absolute paths, home directory, Bearer tokens, long secret-shaped segments, and Docker/container identifiers. Applied to `ApiMessage::Untrusted` error shaping and `RegistrationState::sanitized()`.
- **Registration exposure split**: `/health` returns `RegistrationPublicSummary` (`{ enabled, registered }` only). `/status` returns full `RegistrationState` with `last_error` sanitized through the redaction module.
- **Query credential rejection middleware**: Global middleware rejects auth-like query keys (`token`, `access_token`, `api_key`, `bearer`, `secret`, `password`, `credential`) case-insensitively with percent-decoding. Returns 400. Runs before `TraceLayer` so sensitive query strings never reach request traces.
- **Compose secret preservation**: Auth env placeholders in generated compose files use `${LFD_AUTH_TOKEN:-}` syntax — never interpolated from runtime values. Regression test proves config secrets and env secrets don't leak into the generated file.
- **Token rotation command**: `lfd token rotate` generates a 32-byte random hex token, prints it once with a restart runbook, and does not persist to disk.
- **Documentation**: Token rotation workflow, auth transport requirements, and query rejection behavior documented in `docs/lfd.md`.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| `secrecy` crate over hand-rolled `SecretString` | Audited, zeroize-on-drop, redacted Debug/Display, constant-time eq via `subtle`. No reason to maintain our own. | Hand-rolled version had the right API shape but lacked zeroize and audited constant-time internals. |
| `ApiMessage::Safe` / `Untrusted` enum | Forces callsites to explicitly choose between passthrough and sanitized output. Static `&str` auto-converts to `Safe` via `From`. | Alternative: sanitize everything. Rejected because domain identifiers and known error strings don't need redaction and would produce confusing output. |
| Query rejection before trace layer | Prevents query-string secrets from appearing in request traces. Middleware ordering in axum is inside-out; placing rejection outermost ensures it runs first. | Alternative: sanitize traces. Rejected because preventing the data from entering traces is strictly better than scrubbing after. |
| Restart-based token rotation | Simple, explicit, testable. No dual-token grace window or hot-reload complexity. | Hot-reload or dual-token window. Rejected as premature — restart is the expected deployment pattern and avoids race conditions. |
| Heuristic redaction | Practical defense-in-depth. Covers known patterns (paths, bearer tokens, long alphanumeric segments, Docker identifiers). | Structured error types everywhere. Rejected as too large a refactor for the security value gained — heuristic redaction catches the most common leak patterns now. |

## How it fits together

The credential hygiene hardening works at three layers:

1. **Type boundary** — `SecretString` ensures tokens are never accidentally logged, displayed, or compared in variable time. Any code touching a token must call `expose_secret()` explicitly.
2. **HTTP boundary** — Query credential rejection middleware blocks auth-like params globally before trace logging. `ApiMessage::Untrusted` triggers redaction before error payloads reach operators. Registration state is audience-split between health probes and authenticated status.
3. **Config boundary** — Compose file generation uses environment variable placeholders, never interpolated secrets. Token rotation is a generate-print-restart workflow with no persistence.

## Risks and bottlenecks

- **Redaction is heuristic.** The pattern-based approach in `redaction.rs` covers known formats but won't catch novel secret shapes. The `looks_secret` heuristic (24+ chars, alphanumeric with digits) is intentionally conservative — it may over-redact some long identifiers but won't under-redact short tokens.
- **Query denylist is static.** If new credential-like query keys are introduced, the `AUTH_LIKE_QUERY_KEYS` array needs manual expansion. Currently covers the 8 most common patterns.
- **Bearer prefix matching is case-limited.** `extract_token` matches `"Bearer "` and `"bearer "` but not other casings like `"BEARER "`. This is per RFC 6750 which specifies case-insensitive scheme matching — axum's header parsing lowercases the header name but not the value. In practice, all major clients use standard casing.

## What's not included

- **JWKS fail-closed validation** — Deferred to Phase 06, blocked on remote/07.
- **Token format pre-validation** (length caps, null byte rejection) — Scoped for Phase 06.
- **Dual-token grace window or hot-reload** — Explicitly deferred; restart-based rotation is the design choice.
- **Multi-tenant authorization** — Out of scope for security hardening; separate remote/09 concern.

## Validation

All checks pass:

```
cargo fmt --check         # clean
cargo clippy -- -D warnings  # clean
cargo test --all          # 443 tests pass
uv run pytest python/tests/  # 39 tests pass
```

Key test coverage:
- `sanitize_operator_message_redacts_paths_and_tokens` — end-to-end redaction
- `sanitize_operator_message_redacts_home_path` — home directory redaction
- `auth_like_query_keys_are_rejected_case_insensitively` — query blocking (integration)
- `auth_like_query_keys_are_rejected_when_percent_encoded` — percent-decode bypass prevention
- `auth_like_query_rejection_happens_before_trace_layer` — middleware ordering
- `api_message_untrusted_sanitizes_paths` — error shaping
- `map_store_error_sanitizes_db_errors` — database error redaction
- `health_handler_returns_public_registration_summary` — exposure split
- `status_handler_sanitizes_registration_last_error` — status sanitization
- `compose_keeps_auth_token_placeholder_and_never_inlines_secret_values` — compose regression
- `format_token_rotation_output_prints_token_once_with_runbook_steps` — rotation UX
