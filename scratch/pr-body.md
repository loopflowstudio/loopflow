## Try it!

```bash
# New PM auth commands
lfq auth asana
lfq auth linear
lfq auth status

# Validation
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
```

What to look for:
- `lfq auth status` now includes **Asana** and **Linear**.
- PM API-key providers no longer show misleading `pay-per-token` copy in CLI status/output.
- Rust tests cover PM config parsing, wave `pm:` parsing, and roadmap `pm_id` round-tripping.

## Intent

Lay the foundation for PM integration without jumping ahead to provider clients. This branch introduces the shared PM trait/types, makes PM config parseable in waves and global config, and wires Asana/Linear into the existing auth/token plumbing so later waves can build bootstrap/import/export flows on top of stable primitives.

## Assumptions

- Asana and Linear v1 setup is API-key/PAT based, not browser OAuth.
- Existing encrypted provider-token storage is the right place to keep PM credentials.
- Waves without PM configuration must continue to behave exactly as they do today.

## Key decisions

- Added a dedicated `lfd::pm` module as the single source of truth for PM config/item types and roadmap frontmatter parsing.
- Reused the existing provider auth service and `/auth` endpoints for Asana/Linear instead of creating PM-only credential APIs.
- Kept PM API-key UX separate from metered model-provider UX: Claude/Codex/OpenCode Zen still warn about pay-per-token billing, Asana/Linear do not.

## Not included

- Asana/Linear API clients
- Bootstrap `lf ops asana|linear ...` commands
- `import-pm` / `export-pm`
- ingest sync and PR lifecycle sync
