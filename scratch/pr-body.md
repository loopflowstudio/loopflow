## Try it!

```bash
# Build and launch Concerto
uv run python scripts/concerto-dev.py run-debug

# Observe: daemon starts immediately at launch, sidebar shows "Starting daemon..." spinner
# Observe: provider connections detect local Claude/Codex auth from credential files
```

If you have Claude Code authenticated (`~/.claude/.credentials.json` exists), the GitHub/Claude/Codex provider cards should show detected auth without clicking Connect.

Run the new tests:
```bash
cargo test -p loopflow extract_codex_token_reads_nested  # Rust: nested Codex token
swift test --package-path swift --filter CredentialSocket  # Swift: file credential reader
```

## Intent

Phase 1 of the provider auth redesign. Instead of fake "Connect" buttons that open browser pages the app can't complete, detect credentials that already exist on disk. Instead of waiting for the user to trigger daemon startup, start eagerly at app launch.

The connection handshake for bundled mode is also streamlined — localhost doesn't need TLS trust checks or repo discovery, so those phases are skipped.

## Assumptions

- Claude Code stores credentials at `~/.claude/.credentials.json` with an `accessToken` field.
- Codex CLI stores credentials at `~/.codex/auth.json` with either a top-level `access_token` or nested `tokens.access_token` (ChatGPT auth mode).
- GitHub credentials are in macOS Keychain under the `gh:github.com` service.

## Key decisions

- **Eager daemon start at app launch** rather than on-demand when a repo window opens. Trades a small background process for noticeably faster time-to-interactive.
- **File reading over CLI shelling** for credential detection. Faster, no dependency on CLI binaries being installed.
- **Removed orphan worktree sidebar section.** Low-value UI that added code complexity. Worktrees are managed in terminal.
- **Bundled mode skips TLS and repo discovery phases.** Localhost with a generated token doesn't need either.

## Not included

- Provider-specific auth flows (device code, terminal-assisted login) — Phase 2.
- Typed `AuthStep` model replacing generic `AuthFlow` — Phase 2.
- Provider provenance badges in the UI — deferred.
