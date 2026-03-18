## Try it!

```bash
cargo test -p loopflow -- secrets          # 9 tests: sync, clear, status, smart defaults
swift test --package-path swift            # 252 tests (SecretsProviderTests, SmartDefaultConfigTests, DopplerProject/Config)
```

In Concerto, open any repo's Connection Settings. Providers are now grouped by role (Agents, Source Control, Project Management, Secrets). Doppler appears under Secrets — connect via OAuth, pick a project and config, and watch Claude/Codex keys populate from Doppler. "Refresh" re-syncs. "Disconnect" removes only the keys Doppler supplied.

Portfolio window now has a Connections toolbar button (link icon) that opens a sheet with the same grouped panel.

## Intent

Two pieces that belong together: the secrets provider needed a home in the UI, and the connections panel needed structure. Grouping providers by role makes the panel scannable and gives secrets a natural location instead of a bolted-on section.

## Assumptions

- Doppler CLI is installed and `doppler login` has been run (or user authenticates through the OAuth flow in Concerto)
- Doppler service tokens don't expire — no refresh flow needed
- Only two key mappings today: ANTHROPIC_API_KEY → Claude, OPENAI_API_KEY → Codex

## Key decisions

- **Removed `SecretsProvider` trait** — only one implementation, so direct functions are simpler. The trait can be reintroduced if a second provider materializes.
- **Doppler token in `provider_tokens`** — Doppler is a first-class auth provider now, not a separate credential path. `secrets_provider_config` stores only project/config selection.
- **Auto-persist CLI tokens** — if `doppler login` was run in a terminal, lfd detects the active session and persists the token on next status check.
- **Separate browse/select API** — `GET /secrets/projects`, `GET /secrets/configs?project=`, `POST /secrets/select` instead of a single connect endpoint, so the UI can let users browse before committing.

## Not included

- Per-repo provider enable/disable (infrastructure wired, UI toggle deferred)
- Portfolio-level secrets persistence
- Typed auth steps (separate design at `wave/concerto/05-typed-auth-methods.md`)
