# 01: Deployment Collapse

**Finish line:** `lfd` has three documented deployment profiles (`solo`, `team`, `ci`), and every shipped deploy doc or compose example starts from one of those profiles instead of the current config matrix.

## Carried context

- `LFD_MODE=native|container` already owns `service_manager`, `runtime_backend`, `storage`, and `executor.type`; those keys are rejected if callers try to configure them directly.
- `AuthMode` is now a proper enum (`Local`, `Ci`, `Studio`) with `#[serde(deny_unknown_fields)]` on `AuthConfig`. The old `auth.provider` key is rejected; `auth.mode: static` is accepted but canonicalized to `ci` with a deprecation warning.
- `Mode::Container` currently implies compose + postgres and upgrades `auth.mode=local` to `studio` when no token is configured.
- Remote deploy docs and production compose use `ci` terminology (`LFD_AUTH_PROVIDER=ci`), but operators still assemble deployments env var by env var instead of selecting a profile.

## What to build

1. Decide whether the public entrypoint replaces `mode` or layers a higher-level profile on top of `native|container`. `solo`, `team`, and `ci` must map onto the existing `ModeProfile` logic instead of recreating the old matrix.
2. Encode defaults for auth, storage, runtime backend, executor, and service manager behind those profiles. Keep lower-level overrides as escape hatches, but stop documenting them as the normal path.
3. Update CLI/config/docs/compose so every blessed example starts from one of the three profiles.
4. Remove or hide any remaining configuration shape that pushes users back toward arbitrary combinations.

## Uncertainty

- `ci` may want a headless, non-persistent shape that does not line up exactly with today's `native|container` split.
- `team` may depend on `wave/trust/06-team-auth.md` if self-hosted auth needs to land before the profile is honest.

## Done when

- A single profile selection produces sane defaults for solo, team, and ci deploys.
- Docs and compose examples no longer ask operators to choose auth, storage, and isolation independently.
- Existing local and remote smoke checks still pass with the blessed profiles.
