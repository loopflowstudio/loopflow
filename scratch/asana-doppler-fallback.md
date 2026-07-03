# Asana OAuth client creds self-serve from Doppler

## Problem

`oauth_client_credentials` (`rust/loopflow/src/lfd/provider_auth.rs:1834`) reads
`ASANA_CLIENT_ID` / `ASANA_CLIENT_SECRET` **only** from `std::env::var`. When
they're absent it returns `AuthError::CommandUnavailable` — "set ASANA_CLIENT_ID
and ASANA_CLIENT_SECRET to enable Asana OAuth".

Consequence: `lf op pm` and `lf op auth asana` only work when a human hand-wraps
the invocation in `doppler run -p loopflow -c dev -- ...`. The unattended wave
loop can't read or write its Asana roadmap without that manual wrapper. This is
the exact human-in-the-loop barrier the Systems wave exists to delete: a
credential a CLI could fetch itself.

The creds already live in Doppler (`loopflow` project): `ASANA_CLIENT_ID`,
`ASANA_CLIENT_SECRET`. The codebase already shells out to Doppler elsewhere —
see `extract_doppler_token` and the `Command::new("doppler")` calls in the same
file (~lines 1277–1325).

## Change

When `ASANA_CLIENT_ID` / `ASANA_CLIENT_SECRET` are missing from the environment,
fall back to fetching them from Doppler before erroring. Concretely, in
`oauth_client_credentials` (or a small helper it calls):

1. Try `read_nonempty_env(name)` first (unchanged fast path — an explicit env
   var still wins, so `doppler run` and CI env still work).
2. On miss, fetch from Doppler: `doppler secrets get <NAME> --plain` (respect the
   project — reuse whatever project/config the existing doppler helpers use; if
   they don't pin one, read from the ambient `doppler` config so a repo-level
   `doppler.yaml` or `DOPPLER_PROJECT` governs it). Do **not** print the value.
3. Only if both miss, return the existing `CommandUnavailable` error — but widen
   its message to mention the Doppler fallback was also tried.

Keep the Doppler call non-fatal: if `doppler` isn't installed or returns
non-zero, treat it as a miss and fall through to the existing error, don't panic.

Match the shape of the existing doppler helpers in this file (async `Command`,
same error handling idiom). This is a general fix — it applies to any PM OAuth
provider routed through `oauth_client_credentials`, not just Asana.

## Test

- Unit: with env vars set, the Doppler path is never invoked (env wins).
- Unit: with env vars unset and a stubbed doppler runner returning values, the
  creds resolve from Doppler.
- Unit: with both unset and doppler failing/absent, the original
  `CommandUnavailable` error surfaces (no panic).

Mock the doppler invocation via a closure/injected runner — do **not** reshape
production code with a factory trait just for the test (see CLAUDE.md). If the
existing doppler helpers aren't already testable via a runner seam, prefer a
`#[cfg(test)]` seam over a new public abstraction.

## Out of scope

- Token refresh (`refresh_pm_oauth_token`) already works non-interactively; don't
  touch it.
- Wiring creds into the wave runner's env — the CLI self-serving is the fix; the
  runner shouldn't need to know about Doppler.
