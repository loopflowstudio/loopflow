# Deployment Collapse

## Problem

The deployment surface tells two contradictory stories. Code enforces two strict profiles (`native` and `container`) — `resolve()` rejects direct overrides of `service_manager`, `runtime_backend`, `storage`, and `executor.type`. But docs and compose examples still expose these dimensions as knobs. The daemon reference lists every env var and YAML key without distinguishing "this is a blessed default" from "this is an escape hatch." The dev compose file defaults `LFD_AUTH_MODE` to `local`, contradicting the container/studio shape that `resolve()` auto-promotes to.

Operators read docs, not code. Until the docs match the code's opinion, the deployment surface feels combinatorial even though it isn't.

## Approach

Build on the existing `native|container` profiles. No new mode names. Three passes:

### Pass 1: Align compose examples with the blessed shapes

- `docker/docker-compose.yml`: change `LFD_AUTH_MODE` default from `local` to `studio`. This matches what `resolve()` already does (auto-promote container + local-without-token to studio). The dev compose shouldn't fight the code.
- `deploy/docker-compose.prod.yml`: already sets `LFD_AUTH_MODE: studio`. Drop the redundant override now that the base file agrees.
- Remove commented-out credential mount lines from `docker/docker-compose.yml`. They present manual volume mounts as the path when the config system (`executor.credentials.mounts`) already handles this.
- In blessed remote docs, do not ask operators to set `LFD_AUTH_MODE=studio` in the quick start. Container/studio is the documented default shape, not a choice the recipe should surface.

### Pass 2: Restructure daemon reference docs

Rewrite `docs/lfd.md` to lead with the two shapes:

1. **Native** (default): `lfd install` on macOS/Linux. Sqlite, local auth, local executor. Zero config for solo use.
2. **Container**: `lfd install --mode container` or `LFD_MODE=container`. Postgres, studio auth, Docker executor. For remote/shared access.

Move the full env var and YAML reference into a "Configuration reference" section below the shapes. Label auth, executor, and http_security knobs as what they are: tuning knobs within a shape, not independent deployment choices.

Be explicit about sandbox status: until `wave/clear-the-deck/02-sandbox-pause.md` resolves whether sandbox remains, blessed container docs describe Docker as the default container executor. `executor.sandbox` stays documented only as an experimental override in the reference section, not as part of the main deployment story.

### Pass 3: Simplify deploy/README.md

The remote deploy README should be a recipe, not a configuration guide. It currently duplicates env var docs that belong in the daemon reference. Trim to:
- Prerequisites
- Quick start (one `docker compose` command)
- Verify
- Troubleshoot

Move credential mount guidance to point at `executor.credentials.mounts` in `lfd.yaml` config rather than raw Docker volume lines.
Treat `LFD_AUTH_MODE` the same way: omit it from the happy path, and if it appears at all, place it under an overrides/escape-hatches note rather than the primary recipe.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add `solo/team/ci` profile names | Simpler mental model upfront | Wave item explicitly says don't invent profile names the product can't explain yet. `team` auth doesn't exist (trust/06). `ci` is just `native + LFD_AUTH_TOKEN`. |
| Delete all env var overrides | Forces blessed paths only | Escape hatches are valuable for internal flows and debugging. Hide them from docs, don't remove from code. |
| Merge deploy/README.md into docs/lfd.md | One doc to maintain | Remote deploy is a recipe with prerequisites (EC2, Caddy, DNS). Separate doc is appropriate. |

## Key decisions

**No new mode names.** The wave item is explicit: "without reintroducing speculative profile names that the product does not yet justify." `native` and `container` are the shapes. `team` auth belongs in `wave/trust/06-team-auth.md`. CI usage is just native mode with `LFD_AUTH_TOKEN` — documenting it as a separate mode would be dishonest about what the code does.

**Container defaults to studio auth in compose.** The code already auto-promotes `container + local-without-token` to studio. The compose file should agree rather than setting `local` and relying on the promotion.

**Credential mounts stay as config, not raw volumes.** `executor.credentials.mounts` already validates and maps named mounts. Compose examples should use `LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,ssh` env vars, not hand-written Docker volume lines.

**Blessed container docs describe one backend.** Until sandbox pause lands, the public container story is Docker. `executor.sandbox` remains an experimental override documented only in reference material, not in the front-door deployment narrative.

**Deploy README stays separate but shrinks.** Remote deploy is a distinct use case with its own prerequisites. But it shouldn't duplicate the daemon reference.

**Do it all means standardize on the real entrypoint, not paper over it.** The current CLI does not expose a real `lfd install --container` or `lfd install --mode container` flag. Adding an install-only flag in this pass would create a second problem: `install`, `start`, and `status` would disagree unless the choice is also persisted into config. This pass should therefore remove fictional install flags from docs and standardize on the actual configuration surface (`mode: container` in `~/.lf/lfd.yaml` or `LFD_MODE=container`) everywhere. If we later want install sugar, it should be a coherent CLI feature that writes or updates config rather than a one-shot override.

## Package selection

**Selected package: Do it all**

Land the full coherence pass now:

- Align compose defaults with container/studio.
- Remove `LFD_AUTH_MODE=studio` from blessed remote quick starts.
- Mark `executor.sandbox` as experimental override/reference-only until sandbox pause resolves.
- Sweep all user-facing deployment entry docs (`docs/lfd.md`, `deploy/README.md`, `docs/getting-started.md`, related README mentions) so they teach the same two shapes.
- Remove doc references to unsupported install flags and standardize on the real mode-selection path.

## Scope

- In scope: docs/lfd.md rewrite, deploy/README.md trim, docker-compose file alignment, removal of stale per-dimension config examples
- In scope: making the blessed remote recipe stop surfacing `LFD_AUTH_MODE` as a choice
- In scope: explicit sandbox-status wording in daemon docs so the public container path is unambiguous
- In scope: sweeping user-facing deployment entry docs so unsupported install flags are removed and the entrypoint is consistent
- Out of scope: team auth mode (trust/06), sandbox decisions (clear-the-deck/02-sandbox-pause), new CLI subcommands that introduce a new persistent deploy preset model
- Out of scope: install-only mode flags that do not persist and would leave `start`/`status` inconsistent with the installed backend

## Done when

Wave goals quoted:
> Users choose from a small, honest deployment surface instead of a bag of orthogonal config knobs.
> The default container execution path is obvious in both code and docs.
> Deploy and operator docs describe only blessed paths.

Verification:
- `grep -r 'solo\|team\b\|ci\b' docs/ deploy/ docker/` finds no deployment profile references outside of CI/CD context
- `docker/docker-compose.yml` defaults `LFD_AUTH_MODE` to `studio`
- `deploy/docker-compose.prod.yml` no longer overrides auth mode (redundant with base)
- `docs/lfd.md` opens with two shapes (native, container) before any config reference
- `deploy/README.md` is a recipe with no duplicated env var tables
- `docs/getting-started.md` and other user-facing deploy entry docs no longer mention unsupported install flags
- Blessed remote quick starts no longer require setting `LFD_AUTH_MODE=studio`
- `docs/lfd.md` describes Docker as the blessed container executor and keeps sandbox in override/reference material only
- Existing smoke tests pass: `tests/e2e/test_smoke.sh`, `cargo test -p loopflow docker_`, remote smoke if applicable

## Measure

Line count of user-facing deployment docs before and after:
```bash
wc -l docs/lfd.md deploy/README.md docker/docker-compose.yml
```
Target: fewer total lines, with the two-shape structure front-loaded in the daemon reference.
