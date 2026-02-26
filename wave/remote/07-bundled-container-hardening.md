# 07: Bundled Container Hardening

Status: **next**

Bundle-container mode shipped, but three follow-ups remain before this path is "boring by default."

## Scope

### In scope

1. Replace lightweight socket `POST /auth/{provider}/start` handling with provider-aware auth launch + status tracking.
2. Decide native fallback policy in Concerto (`concerto.bundledDaemon.preferNativeMode`): persisted preference vs one-shot recovery.
3. Resolve or explicitly classify the local `ConcertoUITests-Runner` early-exit crash seen in `xcodebuild test -scheme Concerto`.

### Out of scope

- Studio JWT rollout (covered by `04-studio-auth.md`)
- Remote host lane parity work (covered by `02-mac-mini-dogfood.md`)
- New API surface expansion (covered by `05-api-expansion.md`)

## Done when

- Socket-backed auth start flow is no longer best-effort URL handoff and has clear completion semantics.
- Native fallback behavior is intentional, documented, and reflected in Connection Settings UX.
- Concerto UI test behavior is either stable in this branch or documented as a known external flake with evidence.
