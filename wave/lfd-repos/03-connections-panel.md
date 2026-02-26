# 03: Concerto Connections Panel

Add a Connections UI in Concerto for GitHub/Claude/Codex auth management.

## What to build

- Three provider cards with status and connect/disconnect actions.
- `Connect` opens `verification_uri_complete` (or fallback `verification_uri`) automatically.
- Status updates from auth lifecycle WebSocket events.
- Browser-first UX parity with `lfq auth` flows.

## Depends on

- Phase 02 auth-flow validation.
- Stable `/v0/auth` responses and auth lifecycle events.

## Done when

- Users can connect/disconnect all three providers from Concerto with no terminal steps.
- Card state tracks server auth state and event updates reliably.
