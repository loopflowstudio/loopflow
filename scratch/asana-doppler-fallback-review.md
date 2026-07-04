# Asana Doppler Fallback Review

## What was implemented

Asana OAuth client credentials now resolve from the environment first, then from Doppler via `doppler secrets get <NAME> --plain`, before returning `CommandUnavailable`.

The same credential resolver is used by the Asana OAuth start/complete path and PM OAuth token refresh, so `lf op auth asana` and `lf op pm` can run without a manual `doppler run` wrapper when the local Doppler CLI is configured.

## Key choices

- Environment variables still win. CI and explicit `doppler run` invocations keep their current behavior and avoid spawning Doppler.
- Doppler lookup is a private async runner seam, not a public abstraction. Tests can stub the lookup without adding factory traits or reshaping production API.
- Doppler failures are treated as misses. Missing CLI, non-zero exit, empty output, or absent secrets all fall through to the existing unavailable-credentials error path.
- Secret values are read only from stdout into memory and are never logged or printed.

## How it fits together

`oauth_client_credentials` became async because Doppler lookup shells out through Tokio `Command`. It delegates to `oauth_client_credentials_with_doppler_runner`, which resolves each credential with `read_oauth_client_credential`: read a non-empty env var, otherwise call the injected Doppler fetcher.

`fetch_doppler_secret` mirrors the existing Doppler helpers in `provider_auth.rs`: run the CLI, require success, trim stdout, and return `None` for any miss.

## Risks and bottlenecks

- If both env vars are absent, the OAuth path now may run two Doppler CLI commands. That is acceptable for auth setup and refresh, but it is slower than the env fast path.
- The fallback relies on the ambient Doppler project/config. A misconfigured local Doppler context still produces the same user-facing unavailable-credentials error.
- The error message names Doppler as a fallback, but intentionally does not include Doppler stderr to avoid surfacing secret-adjacent output.

## What's not included

- No token refresh logic changed beyond awaiting the shared credential resolver.
- No wave-runner environment wiring was added.
- No Doppler project/config pinning was added; ambient Doppler configuration remains the source of truth.

## Validation

- `cargo test -p loopflow oauth_client_credentials`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
