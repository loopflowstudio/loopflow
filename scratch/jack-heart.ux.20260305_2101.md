# Provider auth redesign — Phase 1 validation

## Try it

```bash
# Build and launch Concerto
uv run python scripts/concerto-dev.py run-debug

# Observe: daemon starts immediately at launch, sidebar shows "Starting daemon..." spinner
# Observe: provider connections detect local Claude/Codex auth from credential files
```

If you have Claude Code authenticated (`~/.claude/.credentials.json` exists), the GitHub/Claude/Codex provider cards should show detected auth without clicking Connect.

## Tests

```bash
cargo test -p loopflow extract_codex_token_reads_nested  # Rust: nested Codex token
swift test --package-path swift --filter CredentialSocket  # Swift: file credential reader
```

## What to verify

- Eager daemon start: daemon process starts at app launch, not on repo window open
- Credential detection: provider cards show "Connected" for locally-authenticated providers without clicking Connect
- Bundled mode handshake: no TLS trust check or repo discovery phase for localhost
- Sidebar: "Starting daemon..." spinner shown during startup, no orphan worktree section
