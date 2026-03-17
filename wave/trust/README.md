# Trust

## Vision

Safe to leave running overnight. Credentials are encrypted, execution is isolated, identity is verified. You hand loopflow your API keys and walk away.

### Not here

- Test coverage and code cleanup (that's Foundation)
- New features unrelated to trust or remote access
- Product-surface simplification work owned by Clear the Deck

## Strategy

Hardening, credential encryption, and studio auth are shipped.

Two trust tracks remain:

1. **Self-hosted team auth.** Remote hosts still depend on the hosted studio auth service plus valid host credentials. Team mode should let a self-hosted `lfd` own login, refresh, and request validation without falling back to manual bearer-token UX.
2. **Sandbox isolation, if it survives clear-the-deck.** Validation and rollout stay here, but the product-level verdict on whether sandbox remains a blessed path lives in `wave/clear-the-deck/02-sandbox-pause.md`.

## Goals

- Self-hosted teams can authenticate without the hosted studio auth service.
- Agent execution can run in isolated sandboxes when the platform contract is real enough to support.
- Remote auth and execution defaults stay understandable instead of fracturing into special cases.

## Risks

- Team auth touches daemon auth, token issuance, and client sign-in flows at once; a partial rollout can strand remote users.
- Sandbox work can burn time on plugin distribution and DinD quirks even if Docker remains the only honest default.
- Self-hosted operators still need a simple, supportable secret-management story.

## Metrics

- Self-hosted remote deploys that require the hosted studio auth service after team mode: 0
- Sandbox executor parity with Docker executor on supported platforms: 100%
- Platform validation pass rate across supported sandbox hosts: 100%
