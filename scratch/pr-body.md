## Try it!

```bash
cargo test -p loopflow -- secrets          # 5 tests: sync, clear, status
swift test --package-path swift            # SecretsProviderTests suite (6 tests)
```

In Concerto, open Connection Settings → Secrets Provider section. Enter a Doppler service token, project, and config. On connect, Claude and Codex keys populate from Doppler. "Refresh" re-syncs. "Disconnect" removes only the keys Doppler supplied — manually-entered keys stay.

## Intent

Users shouldn't paste API keys by hand. This adds a secrets provider abstraction (Doppler first) so lfd can fetch and sync Claude/Codex credentials automatically. Concerto shows which keys are present and which are missing.

## Assumptions

- lfd runs locally with SQLite — the Postgres secrets store is stubbed
- Doppler service tokens (not OAuth device flow) are used for auth
- Only two key mappings: ANTHROPIC_API_KEY → Claude, OPENAI_API_KEY → Codex

## Key decisions

- **"via " prefix as ownership marker** — tokens supplied by secrets providers get `login: "via doppler"`. Disconnect only clears these, leaving manually-entered keys intact. No extra schema column needed.
- **Shared SwiftUI view** — `SecretsProviderSection` in LoopflowCore, used by both macOS and iOS without `#if` guards.
- **Reuse existing credential path** — synced keys are stored as regular `ProviderToken` entries with `credential_type: ApiKey`. No parallel storage system.

## Not included

- CLI commands (`lf auth doppler`) — next step
- Doppler device-flow OAuth — uses service tokens for now
- Periodic auto-sync — manual refresh only
- 1Password / Vault providers
