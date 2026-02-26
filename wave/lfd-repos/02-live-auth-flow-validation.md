# 02: Live Auth-Flow Validation

Validate provider auth behavior against real CLI output and credential files.

## Why this phase exists

Phase 01 shipped parser- and filesystem-driven auth flows with mocked/unit coverage. Remaining risk is contract drift against live provider CLIs.

## Scope

### In scope

- Run live `gh`, `claude`, and `codex` auth flows through `POST /v0/auth/:provider`.
- Confirm URL capture timing and payload fields (`verification_uri(_complete)`, `user_code`).
- Confirm `GET /v0/auth` transitions and `auth.connected`/`auth.failed` events.
- Confirm Claude disconnect removes auth credentials without deleting user settings.

### Out of scope

- Concerto UI work.
- New providers.
- Token refresh logic (provider-owned).

## Done when

- Live auth passes for all three providers without manual URL copy/paste hacks.
- Claude disconnect behavior is validated against real credential layouts.
- Any parser/heuristic changes needed by live validation are codified with regression tests.
