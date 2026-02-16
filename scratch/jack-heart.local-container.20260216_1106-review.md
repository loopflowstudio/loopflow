# lfd: strict mode profiles and compose service backend

## What was implemented

Replaces the previous ad-hoc `LFD_STORAGE`, `LFD_EXECUTOR_TYPE`, and individual config knobs with a single `mode` field (`native` or `container`) that selects a strict profile. Each profile locks down four coupled settings — `service_manager`, `runtime_backend`, `storage`, and `executor.type` — so they can't drift out of sync.

Adds a compose service backend: when `mode: container`, `lfd install` generates `~/.lf/docker-compose.yml`, pulls images, and configures launchd/systemd to run `docker compose up` instead of the native binary. Install/start detect backend mismatches and require `--force` to switch.

Also adds worktree freshness detection — branches with no commits beyond main are marked "fresh" and prunable by `lf ops wt list`.

## Key choices

- **Strict profiles over flexible config**: Users set `mode: native` or `mode: container`. Attempting to set `storage`, `runtime_backend`, `service_manager`, or `executor.type` directly in YAML is a hard error. This eliminates an entire class of misconfiguration (e.g. sqlite + docker executor, or postgres + local executor).

- **`RawLfdConfig` → `LfdConfig` resolution**: Deserialization goes through a raw struct that allows `Option` fields for the mode-locked settings. `resolve()` validates nothing was explicitly set, then stamps in the profile. This keeps serde concerns (what's in the YAML) separate from runtime concerns (what the daemon uses).

- **Backend detection from installed artifacts**: Rather than persisting the backend mode in a separate file, `installed_backend()` reads the existing plist/unit file content to detect whether it was configured for compose or native. This is resilient — even if the config file changes, the installed service's actual backend is always known.

- **Compose override via standard docker-compose convention**: `~/.lf/docker-compose.override.yml` uses Docker's native merge semantics rather than inventing a custom extension mechanism.

- **`LFD_MODE` is the only identity env var**: Mode can be overridden via env for container deployments (the docker-compose.yml sets `LFD_MODE: container`). Other profile-locked fields cannot be overridden via env, keeping the profile invariant intact even under env var pressure.

## How it fits together

`LfdConfig::load()` reads `~/.lf/lfd.yaml` → `RawLfdConfig`, applies env overrides, then calls `resolve()` which rejects any explicit profile-locked fields and stamps in the `ModeProfile`. The resolved `LfdConfig` is passed to all service management functions.

Service `mod.rs` loads config and dispatches to `macos.rs` or `linux.rs` based on `config.service_manager`. Both platform modules share `compose.rs` for docker-compose file generation, image pulling, status reporting, and teardown.

## Risks and bottlenecks

- **`docker compose ps --format json` output format varies by version**: The parser handles both array and NDJSON formats, but Docker Compose v1 (hyphenated `docker-compose`) is not supported — the code assumes `docker compose` (v2 plugin).

- **Compose file is a long format string**: `render_compose_file` builds YAML via string formatting. This works but is fragile for additions — an indentation error would produce invalid YAML. A serde-based approach would be safer but heavier.

- **`installed_backend()` heuristic**: Detection relies on string matching (`docker compose` in unit content, `<string>docker</string>` + `<string>compose</string>` in plist). This is correct for the generated output but could false-positive if someone manually edits the service files.

## What's not included

- No migration path for existing installs — users with the old `LFD_STORAGE`/`LFD_EXECUTOR_TYPE` env vars will get errors pointing them to `mode`.
- No `lfd upgrade` command to handle the config transition automatically.
- No tests for the compose file rendering (the guards inside `render_compose_file` are covered by profile invariants, but the YAML template itself isn't validated).
