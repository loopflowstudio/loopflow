# Rust Roadmap: lf Client Refactor (Stage 4)

Make `lf` a protocol client that can target local or remote engines.

## Goal
Keep the CLI UX but move execution to the protocol-first engine, enabling remote control and managed clusters.

## Scope
- Client config for target engine (local/remote)
- Authn for API keys and tokens
- Mapping existing commands to protocol calls
- Event streaming to terminal
- Local standalone mode without requiring `lfd`
- Local mode uses direct `lf` ↔ `lfd-core` integration
 - Remote mode switches `lf` engine to `lfd` that exposes the same subset of `lfd-core` APIs used by `lf`

## Non-goals
- Removing Python immediately
- Rewriting all UX flows

## UX principles
- `lf` behaves the same whether local or remote.
- Clear, actionable errors on auth or protocol mismatch.
- Local mode remains the default for dev.
- Users can run `lf` without installing or running `lfd`.
- Local mode should not require a daemon process.
 - Remote mode should be a pure engine switch, not a UX switch.

## Success criteria
- `lf run` works identically against local and remote.
- Concerto and `lf` can connect to the same daemon.
- Users can opt into remote with a single config change.
- Hosted `lfd` can be targeted from a local `lf` without special flags.
- Local `lf` works out of the box with no daemon running.
 - Remote `lf` uses the same engine API surface as local `lf` (subset parity).

## Open questions
- How should credentials be stored (keychain vs file)?
- Do we need offline mode with cached flows?
