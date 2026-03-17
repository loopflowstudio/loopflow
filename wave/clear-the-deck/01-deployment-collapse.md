# 01: Deployment Collapse

**Finish line:** `lfd` exposes a smaller public deployment story — native/local and container/studio — without reintroducing speculative profile names that the product does not yet justify.

## Carried context

- `mode: native|container` still owns `service_manager`, `runtime_backend`, `storage`, and `executor.type`; those keys are rejected if callers try to configure them directly.
- `auth.mode` / `LFD_AUTH_MODE` now accept only `local` and `studio`. `auth.provider`, `LFD_AUTH_PROVIDER`, and the pre-shared `ci` bearer-token mode are rejected.
- `setup_auth()` now writes explicit `auth.token` / `LFD_AUTH_TOKEN` overrides through the same session-token path used for generated local tokens, so local and studio loopback auth share one persistence path.
- User-facing remote docs now point at `lfd install --container` plus `LFD_AUTH_MODE=studio`, and bundled iOS setup only supports discovery. The public story is smaller, but the daemon reference and generic compose path still surface lower-level auth overrides.
- Remote self-hosted deployments still assume the host can sign into studio (`~/.lf/credentials.json`) and register successfully. Any operator scripts that still set `auth.mode: ci` or `LFD_AUTH_PROVIDER` now fail fast.

## What to build

1. Decide whether `mode` alone is the public entrypoint or whether install/docs need a higher-level deploy preset that hides auth overrides without inventing a new matrix.
2. Make every blessed container example start from the same container/studio shape. Leave local-token container launches as escape hatches, not the documented default.
3. Rewrite daemon reference and onboarding docs so they teach the two supported shapes first and label lower-level auth knobs as overrides.
4. Remove the remaining examples or scripts that still present auth, storage, and isolation as independent user choices.

## Uncertainty

- `team` may need to land in `wave/trust/06-team-auth.md` before a richer self-hosted remote story is honest.
- If local-token container launches remain useful for bundled or internal flows, the docs need a crisp distinction between supported defaults and internal overrides.

## Done when

- Native/local and container/studio are the only documented deploy shapes until stronger profile semantics exist.
- Docs and compose examples no longer ask operators to choose auth, storage, and isolation independently.
- Existing local and remote smoke checks still pass with the blessed profiles.
