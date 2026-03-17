# 01: Deployment Collapse

**Finish line:** `lfd` exposes a smaller public deployment story — native/local and container/studio — without reintroducing speculative profile names that the product does not yet justify.

## Carried context

- `LFD_MODE=native|container` already owns `service_manager`, `runtime_backend`, `storage`, and `executor.type`; those keys are rejected if callers try to configure them directly.
- `AuthMode` is now a proper enum (`Local`, `Studio`) with `#[serde(deny_unknown_fields)]` on `AuthConfig`. The old `auth.provider` key is rejected, and the pre-shared bearer-token mode is gone.
- `Mode::Container` currently implies compose + postgres and upgrades `auth.mode=local` to `studio` when no token is configured.
- Remote deploy docs and production compose now point at `studio` auth, but operators still assemble deployments env var by env var instead of selecting a simpler public entrypoint.

## What to build

1. Decide whether the public entrypoint replaces `mode` or layers a higher-level profile on top of `native|container`. If profiles land, they should map onto the existing `ModeProfile` logic instead of recreating the old matrix.
2. Encode defaults for auth, storage, runtime backend, executor, and service manager behind the existing native/container split. Keep lower-level overrides as escape hatches, but stop documenting them as the normal path.
3. Update CLI/config/docs/compose so every blessed example starts from one of the supported public shapes instead of mixing and matching auth knobs.
4. Remove or hide any remaining configuration shape that pushes users back toward arbitrary combinations.

## Uncertainty

- `team` may depend on `wave/trust/06-team-auth.md` if self-hosted auth needs to land before a richer remote story is honest.

## Done when

- Native/local and container/studio are the only documented deploy shapes until stronger profile semantics exist.
- Docs and compose examples no longer ask operators to choose auth, storage, and isolation independently.
- Existing local and remote smoke checks still pass with the blessed profiles.
