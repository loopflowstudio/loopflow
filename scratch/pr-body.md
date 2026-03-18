## Try it!

```bash
cargo test -p loopflow -- secrets          # 5 tests: sync, clear, status
swift test --package-path swift            # SecretsProviderTests suite (6 tests)
```

In Concerto, open Connection Settings → Secrets Provider section. Enter a Doppler service token, project, and config. On connect, Claude and Codex keys populate from Doppler. "Refresh" re-syncs. "Disconnect" removes only the keys Doppler supplied — manually-entered keys stay.
