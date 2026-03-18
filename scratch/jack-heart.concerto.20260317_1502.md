# Validation: Connections Panel + Secrets Provider

## Try it

```bash
cargo test -p loopflow -- secrets          # 9 tests: sync, clear, status, smart defaults
swift test --package-path swift            # 252 tests (SecretsProviderTests, SmartDefaultConfigTests, DopplerProject/Config)
```

In Concerto, open any repo's Connection Settings. Providers are now grouped by role (Agents, Source Control, Project Management, Secrets). Doppler appears under Secrets — connect via OAuth, pick a project and config, and watch Claude/Codex keys populate from Doppler. "Refresh" re-syncs. "Disconnect" removes only the keys Doppler supplied.

Portfolio window now has a Connections toolbar button (link icon) that opens a sheet with the same grouped panel.
