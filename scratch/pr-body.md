## Try it!

```bash
cargo test -p loopflow oauth_client_credentials
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

With `ASANA_CLIENT_ID` and `ASANA_CLIENT_SECRET` absent from the environment but present in the active Doppler config, `lf op auth asana` and PM OAuth refresh can resolve the client credentials without wrapping the command in `doppler run`.

## Intent

Remove the manual Doppler wrapper requirement from Asana OAuth client credential lookup. The CLI now tries explicit env vars first, then quietly asks Doppler for the same secret names, and only then reports unavailable credentials.

## Assumptions

The local Doppler CLI is the right boundary for this fallback. It uses the ambient project/config, so repo-level Doppler config or `DOPPLER_PROJECT`/`DOPPLER_CONFIG` continues to decide where the secrets come from.

## Key decisions

- Keep env vars as the fast path so CI and explicit overrides still win.
- Treat Doppler errors as misses instead of surfacing CLI failures from the OAuth flow.
- Use a private async runner seam for tests rather than adding a public provider abstraction.
- Never log or print fetched secret values.

## Not included

- No changes to the wave runner environment.
- No token refresh behavior changes beyond sharing the async credential resolver.
- No Doppler project/config pinning.
