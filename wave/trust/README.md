# Trust

## Vision

Safe to leave running overnight. Credentials are encrypted, execution is isolated, identity is verified. You hand loopflow your API keys and walk away.

### Not here

- Test coverage and code cleanup (that's Foundation)
- New features
- API completeness

## Strategy

Start with a broad security survey (hardening), then protect credentials at rest (encryption), then replace static tokens with real identity (studio auth), then isolate execution (sandboxes — when unblocked).

## Goals

- No credentials visible in Debug output
- API keys encrypted at rest in lfd's database
- Studio identity replaces static tokens for remote auth
- Agent execution in isolated sandboxes (when Docker Sandbox CLI is available)

## Risks

- **Credential encryption key management.** Where does the decryption key live? macOS Keychain, filesystem, derived from password? Wrong choice is hard to migrate.
- **Studio auth is cross-repo.** Changes must land in both loopflow and studio simultaneously.
- **Sandboxes are blocked.** DinD validation needs Docker Sandbox CLI plugin in the lfd container image.
- **Docker executor per-provider DB calls.** Each container launch makes 3 credential lookups. Batching + encryption should land together.

## Metrics

- Number of credential fields visible in Debug output (target: 0)
- % of token expiry events that resolve via automatic refresh (target: 100% for GitHub/Codex)
- Studio-auth end-to-end success rate on both remote lanes (target: >99%)
- Sandbox executor parity with Docker executor (target: identical behavior)
