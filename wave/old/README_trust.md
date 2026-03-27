# Trust

## Vision

Safe to leave running overnight. Credentials are encrypted, execution is isolated, identity is verified. You hand loopflow your API keys and walk away.

### Not here

- Test coverage and code cleanup (that's Foundation)
- New features
- API completeness

## Strategy

Hardening, credential encryption, and studio auth are shipped. Remaining work is sandbox isolation — blocked on Docker Sandbox CLI plugin availability in Linux containers.

## Goals

- Agent execution in isolated sandboxes (when Docker Sandbox CLI is available)

## Risks

- **Key loss = token loss.** Encryption key lives in platform keychain (macOS Keychain / Linux secret-tool) with file fallback at `~/.lf/provider-token.key`. If both are lost, encrypted tokens are unrecoverable. No key rotation mechanism yet.
- **Sandboxes are blocked.** DinD validation needs Docker Sandbox CLI plugin in the lfd container image.
- **Keychain prompts on macOS.** First `security add-generic-password` may trigger a Keychain Access dialog in non-headless contexts. Fallback path handles this but sandbox/DinD contexts need testing.

## Metrics

- Sandbox executor parity with Docker executor (target: identical behavior)
- Platform validation pass rate across macOS/Linux (target: 100%)
